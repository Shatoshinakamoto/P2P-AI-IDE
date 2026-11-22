/**
 * NodeHostPanel.tsx -- the "Node extensions" part of the Extensions view (phase I6c-2,
 * docs/features/node-extensions-design.md).
 *
 * Purpose: shows whether Node extensions can run for the open folder and lets the person trust the folder, start
 * the Node extension host (downloading it once, after saying so), stop it, and take trust back. Starting or stopping
 * reloads the window, because the editor services connect to the host only when they start (nodeHost.ts).
 *
 * Trust boundary: the shell decides everything (trust store, pinned download, folder from the OS dialog); this panel
 * sends only booleans from the person's clicks.
 * Security: trusting is a separate, explained click before any Node code can run (CWE-829, NIST AC-3); the download
 * needs its own click and names its size (no silent downloads, CWE-494 context); taking trust back stops the host
 * at once in the shell. Phase I6d: the Electron compatibility switch is off by default and is the person's own click;
 * the extra download is named (version and size) before the click that allows it. Extension names in the hint are
 * drawn as React text (CWE-79).
 * Dependencies: React.
 */
import { useEffect, useState } from "react";
import { nodeHostElectron, nodeHostStart, nodeHostStatus, nodeHostStop, nodeHostTrust, type NodeHostStatus } from "../ipc.ts";
import { editorVsCode, hostMatchesEditor, nodeHost, reloadForNodeHost } from "../nodeHost.ts";

interface Props {
  /** How many enabled Node extensions are installed. */
  nodeCount: number;
  onNotice: (m: string) => void;
}

export function NodeHostPanel({ nodeCount, onNotice }: Props) {
  const [st, setSt] = useState<NodeHostStatus | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      setSt(await nodeHostStatus());
    } catch {
      setSt(null);
    }
  };
  useEffect(() => { void refresh(); }, [nodeCount]);

  const run = async (fn: () => Promise<unknown>, reload: boolean) => {
    setBusy(true);
    try {
      await fn();
      if (reload) reloadForNodeHost();
      else await refresh();
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const connected = nodeHost() !== null;
  // Phase I6d: what a start would still have to download, named before the click that allows it.
  const needElectron = !!st && st.electron.enabled && !st.electron.installed;
  const needDownload = !!st && (!st.installed || needElectron);
  const downloads = st
    ? [
        st.installed ? "" : "the Node extension host (about 90 MB)",
        needElectron ? `Electron ${st.electron.version} (about 150 MB)` : "",
      ].filter(Boolean)
    : [];
  const btn = "mt-1 mr-1 rounded border border-ide-line px-1.5 text-xs hover:bg-ide-hover disabled:opacity-50";
  let body: React.ReactNode;
  if (!st) body = <p>Open a folder to use Node extensions.</p>;
  else if (!st.pinned) body = <p>Node extensions can't run on this computer yet: the Node extension host is not available for it.</p>;
  else if (!hostMatchesEditor(st.vscode))
    // Offered only when the server speaks this editor's protocol (lib/vscodeVersion.ts); nothing is downloaded.
    body = (
      <p>
        Node extensions can't run with this version of the IDE yet: the editor is built on VS Code {editorVsCode || "?"},
        but the available Node extension host is built on VS Code {st.vscode}. They must match. Web extensions still work.
      </p>
    );
  else if (!st.trusted)
    body = (
      <>
        <p>
          Node extensions run with your own rights: they can read and change any of your files and run programs. They
          start only in a folder you trust. Trust this folder only if you trust its files and your Node extensions.
        </p>
        <button type="button" className={btn} disabled={busy} onClick={() => void run(() => nodeHostTrust(true), false)}>
          Trust this folder for Node extensions
        </button>
      </>
    );
  else if (!connected)
    body = (
      <>
        <p>This folder is trusted.{needDownload ? ` Starting downloads ${downloads.join(" and ")} once, each checked against its pinned SHA-256.` : ""} The window reloads to connect it.</p>
        <button type="button" className={btn} disabled={busy} onClick={() => void run(() => nodeHostStart(needDownload), true)}>
          {needDownload ? "Download and start Node extensions" : "Start Node extensions"}
        </button>
        <button type="button" className={btn} disabled={busy} onClick={() => void run(() => nodeHostTrust(false), false)}>
          Stop trusting this folder
        </button>
      </>
    );
  else
    body = (
      <>
        <p>Node extensions are running for this folder{st.electron.running ? ` with Electron ${st.electron.version} (compatibility)` : ""}. Changes to them take effect after a restart.</p>
        <button type="button" className={btn} disabled={busy} onClick={() => void run(async () => { await nodeHostStop(); await nodeHostStart(false); }, true)}>
          Restart Node extensions
        </button>
        <button type="button" className={btn} disabled={busy} onClick={() => void run(() => nodeHostStop(), true)}>
          Stop Node extensions
        </button>
        <button type="button" className={btn} disabled={busy} onClick={() => void run(() => nodeHostTrust(false), true)}>
          Stop trusting this folder
        </button>
      </>
    );

  // Phase I6d: the compatibility switch, once Node extensions can run here at all.
  const compat =
    st && st.pinned && st.electron.available && hostMatchesEditor(st.vscode) ? (
      <div aria-label="Electron compatibility" role="group" className="mt-1 border-t border-ide-line pt-1">
        {st.electron.needed_by.length > 0 && !st.electron.enabled && (
          <p>
            These extensions have native modules built for one runtime, usually VS Code's Electron, and may not start
            without compatibility: {st.electron.needed_by.join(", ")}.
          </p>
        )}
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={st.electron.enabled}
            disabled={busy}
            onChange={(e) => {
              const on = e.currentTarget.checked;
              // A running host is stopped by the shell; reload so the editor drops its connection.
              void run(() => nodeHostElectron(on), connected);
            }}
          />
          Run Node extensions with Electron {st.electron.version} (compatibility)
        </label>
        <p className="text-ide-dim">
          For extensions whose native modules only load in VS Code's own Electron. Off by default.
          {st.electron.installed ? "" : ` Downloads Electron ${st.electron.version} from Electron's GitHub releases once (about 150 MB), checked against its pinned SHA-256.`}
        </p>
      </div>
    ) : null;

  return (
    <section aria-label="Node extensions" className="flex flex-col gap-1 rounded border border-ide-line p-2 text-xs text-ide-text">
      <h2 className="text-xs uppercase tracking-wide text-ide-dim">Node extensions ({nodeCount})</h2>
      {body}
      {compat}
    </section>
  );
}
