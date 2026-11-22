/**
 * monaco.ts -- starts the editor: VS Code's own editor core and workbench services (@codingame/monaco-vscode-api),
 * the built-in language and theme extensions, and the workers they need (phase I6a).
 *
 * Purpose: one import point. `startEditor()` must finish before any editor is created (main.tsx awaits it); after
 * that `monaco` is the familiar monaco-editor API (the package name is aliased to @codingame/monaco-vscode-editor-api).
 *
 * Inputs and trust boundaries: only bundled code and the built-in extensions shipped in the app; extensions the person
 * installs are added later by extensions.ts.
 * Security: workers are bundled files (worker-src 'self'; CWE-829); the TextMate engine's WebAssembly is a bundled
 * file too, which is why the CSP allows 'wasm-unsafe-eval' (only WebAssembly compilation, never JS eval). Nothing here
 * fetches from the network. DOMPurify inside the package is the patched 3.4.18 (npm override, CWE-79).
 * Dependencies: @codingame/monaco-vscode-api 39.0.5 and its service overrides and default extensions (MIT, pinned),
 * chosen in ADR-029 D1 because they package VS Code's own extension services for use outside Electron.
 */
import * as monaco from "monaco-editor";
import { initialize } from "@codingame/monaco-vscode-api";
import { IWorkbenchThemeService, StandaloneServices } from "@codingame/monaco-vscode-api/services";
import getExtensionServiceOverride from "@codingame/monaco-vscode-extensions-service-override";
import getTextmateServiceOverride from "@codingame/monaco-vscode-textmate-service-override";
import getThemeServiceOverride from "@codingame/monaco-vscode-theme-service-override";
import getLanguagesServiceOverride from "@codingame/monaco-vscode-languages-service-override";
import type { IWorkbenchConstructionOptions } from "@codingame/monaco-vscode-api/services";
import { SyncDescriptor } from "@codingame/monaco-vscode-api/vscode/vs/platform/instantiation/common/descriptors";
import { ExtensionStorageService } from "@codingame/monaco-vscode-api/vscode/vs/platform/extensionManagement/common/extensionStorage";
import { IExtensionStorageService } from "@codingame/monaco-vscode-api/vscode/vs/platform/extensionManagement/common/extensionStorage.service";
import type { NodeHostConnection } from "./ipc.ts";
import { authorityOf, remotePath } from "./nodeHost.ts";
import getConfigurationServiceOverride, {
  initUserConfiguration,
  updateUserConfiguration,
} from "@codingame/monaco-vscode-configuration-service-override";
// Built-in extensions: VS Code's default themes and the grammars of common languages (declarative, no code runs).
import "@codingame/monaco-vscode-theme-defaults-default-extension";
import "@codingame/monaco-vscode-javascript-default-extension";
import "@codingame/monaco-vscode-typescript-basics-default-extension";
import "@codingame/monaco-vscode-json-default-extension";
import "@codingame/monaco-vscode-css-default-extension";
import "@codingame/monaco-vscode-html-default-extension";
import "@codingame/monaco-vscode-markdown-basics-default-extension";
import "@codingame/monaco-vscode-python-default-extension";
import "@codingame/monaco-vscode-rust-default-extension";
import "@codingame/monaco-vscode-go-default-extension";
import "@codingame/monaco-vscode-yaml-default-extension";
import "@codingame/monaco-vscode-shellscript-default-extension";
import "@codingame/monaco-vscode-xml-default-extension";
import "@codingame/monaco-vscode-ini-default-extension";
import "@codingame/monaco-vscode-docker-default-extension";
import "@codingame/monaco-vscode-cpp-default-extension";
import "@codingame/monaco-vscode-java-default-extension";
import "@codingame/monaco-vscode-powershell-default-extension";
import "@codingame/monaco-vscode-sql-default-extension";
import "@codingame/monaco-vscode-bat-default-extension";
import "@codingame/monaco-vscode-csharp-default-extension";
import "@codingame/monaco-vscode-less-default-extension";
import "@codingame/monaco-vscode-scss-default-extension";
import "@codingame/monaco-vscode-lua-default-extension";
import "@codingame/monaco-vscode-php-default-extension";
import "@codingame/monaco-vscode-ruby-default-extension";
import "@codingame/monaco-vscode-swift-default-extension";

/**
 * The theme the IDE opens with: VS Code's Dark Modern, from the theme-defaults extension. Use the theme's current
 * settings id: an older id ("Default Dark Modern") makes the theme service migrate the setting, and that write fails
 * in the standalone editor ("Model not found").
 */
export const DEFAULT_THEME = "Dark Modern";

// Phase I6b: the web extension host. VS Code runs it in a Worker started from a hidden iframe; both files are bundled
// by Vite (`?worker&url` builds the worker with its imports; the package itself emits the iframe page) and served
// from 'self'.
// The entry module loads extNetShim.ts first (internet only through the IDE for allowed extensions), then VS Code's
// worker; extNet.ts is the page's side and must listen before the host starts.
import extHostWorkerUrl from "./extHostWorker.ts?worker&url";
import "./extNet.ts";

const workerUrls: Record<string, string> = {
  extensionHostWorkerMain: extHostWorkerUrl,
};

// The services ask this hook for a worker by label. Unknown labels get the plain editor worker.
const workers: Record<string, () => Worker> = {
  TextEditorWorker: () =>
    new Worker(new URL("monaco-editor/esm/vs/editor/editor.worker.js", import.meta.url), { type: "module" }),
  TextMateWorker: () =>
    new Worker(new URL("@codingame/monaco-vscode-textmate-service-override/worker", import.meta.url), { type: "module" }),
};
self.MonacoEnvironment = {
  getWorker(_id: string, label: string): Worker | undefined {
    // Labels served by URL (the extension host) are started by VS Code itself from getWorkerUrl.
    if (label in workerUrls) return undefined;
    return (workers[label] ?? workers.TextEditorWorker)();
  },
  getWorkerUrl(_id: string, label: string): string | undefined {
    return workerUrls[label];
  },
  // The bundled extension host worker is an ES module, so the iframe must start it with import(), not importScripts().
  getWorkerOptions(_id: string, label: string): WorkerOptions | undefined {
    return label in workerUrls ? { type: "module" } : undefined;
  },
};

let started: Promise<void> | null = null;


/**
 * Starts the services once; later calls return the same promise.
 *
 * `node` (phase I6c-2) is the running Node extension host for the open, trusted folder, or null. With it, the
 * services connect to that server on 127.0.0.1 with its token, and the folder is the server's workspace, so Node
 * extensions see it. Without it nothing is connected and only web and declarative extensions run (deny by default).
 */
export function startEditor(node: NodeHostConnection | null = null): Promise<void> {
  started ??= (async () => {
    // The theme is a workbench setting; it is set before the services start so the first paint is dark.
    await initUserConfiguration(JSON.stringify({ "workbench.colorTheme": DEFAULT_THEME }));
    let options: IWorkbenchConstructionOptions | undefined;
    let remote = {};
    if (node) {
      // Loaded only when a host runs: importing it registers VS Code's remote contributions, which must not exist
      // (and can't work) in a window without a server.
      const { default: getRemoteAgentServiceOverride } = await import("@codingame/monaco-vscode-remote-agent-service-override");
      // The server's own extensions (the person's Node extensions) are listed from the server.
      remote = getRemoteAgentServiceOverride({ scanRemoteExtensions: true });
      const authority = authorityOf(node);
      options = {
        remoteAuthority: authority,
        // SECURITY: the token from the shell, kept in memory and sent only to the server on 127.0.0.1 (CWE-306).
        connectionToken: node.token,
        // The server checks that the editor's commit is its own; the pin says which one it is.
        ...(/^[0-9a-f]{40}$/.test(node.commit) ? { productConfiguration: { commit: node.commit } } : {}),
        workspaceProvider: {
          trusted: true,
          workspace: { folderUri: monaco.Uri.from({ scheme: "vscode-remote", authority, path: remotePath(node.folder) }) },
          // SECURITY: extensions can't make the window open other folders on the server (CWE-610).
          open: async () => false,
        },
      };
    }
    await initialize(
      {
        ...getConfigurationServiceOverride(),
        ...getExtensionServiceOverride({ enableWorkerExtensionHost: true }),
        ...getTextmateServiceOverride(),
        ...getThemeServiceOverride(),
        ...getLanguagesServiceOverride(),
        // Extensions keep small state (`globalState`, `workspaceState`) through this service; without it the first
        // write from a Node extension (Git does one at start) fails with "not supported". It stores in the editor's
        // in-memory storage, so that state lasts until the window reloads (found with the real Node host, I6c-4).
        [IExtensionStorageService.toString()]: new SyncDescriptor(ExtensionStorageService, [], true),
        ...remote,
      },
      undefined,
      options,
    );
  })();
  return started;
}

/**
 * Switches the colour theme (an id from a built-in or installed theme extension) and resolves once the editor shows
 * it, or after 3 s if the theme never loads (an unknown id leaves the current theme). Waiting matters before an
 * extension is removed: if its theme were still showing, the theme service would fall back on its own and try to
 * rewrite the settings file, which the standalone editor cannot do.
 */
export async function setColorTheme(id: string): Promise<void> {
  const themes = StandaloneServices.get(IWorkbenchThemeService);
  const shown = new Promise<void>((resolve) => {
    if (themes.getColorTheme().settingsId === id) return resolve();
    const timer = setTimeout(() => { sub.dispose(); resolve(); }, 3000);
    const sub = themes.onDidColorThemeChange((t) => {
      if (t.settingsId === id) { clearTimeout(timer); sub.dispose(); resolve(); }
    });
  });
  await updateUserConfiguration(JSON.stringify({ "workbench.colorTheme": id }));
  await shown;
}

export { monaco };
