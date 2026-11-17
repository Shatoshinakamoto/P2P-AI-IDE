/**
 * App.tsx -- the IDE window: title bar, activity bar, side bar, editor area and status bar (VS Code's layout).
 *
 * Purpose: phase I1 of docs/features/editor-shell.md (open a folder through the OS picker in Rust, browse, search,
 * edit in tabs, save) and phase I2 of docs/features/gateways-and-chat.md (Gateways view, Agent chat with history,
 * gateway chip in the status bar) and phase I3 of docs/features/agent-tools.md (chat tools on the open folder and
 * the diff review of proposed changes over the editor area). Missions and Extensions are visible but arrive in later phases.
 * Security: the webview never chooses the folder path (the Rust side shows the OS picker) and never touches files
 * itself; every call goes through ipc.ts to the checked core. All text is rendered by React or Monaco as text (CWE-79).
 */
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import * as Tooltip from "@radix-ui/react-tooltip";
import { Blocks, Files, FolderOpen, MessageSquare, Network, Rocket, Search } from "lucide-react";
import { gatewaysList, pickFolder, workspaceInfo, type GatewayList, type WorkspaceInfo } from "./ipc.ts";
import { Explorer } from "./components/Explorer.tsx";
import { SearchView } from "./components/SearchView.tsx";
import { Editor, type EditorHandle } from "./components/Editor.tsx";
import { StatusBar } from "./components/StatusBar.tsx";
import { Placeholder } from "./components/Placeholder.tsx";
import { NetworkView } from "./components/NetworkView.tsx";
import { ChatView, type Review } from "./components/ChatView.tsx";
import { DiffReview } from "./components/DiffReview.tsx";
import { Tip } from "./components/Tip.tsx";

type View = "explorer" | "search" | "chat" | "missions" | "network" | "extensions";

const VIEWS: { id: View; label: string; icon: ReactNode }[] = [
  { id: "explorer", label: "Explorer", icon: <Files size={22} aria-hidden /> },
  { id: "search", label: "Search", icon: <Search size={22} aria-hidden /> },
  { id: "chat", label: "Agent chat", icon: <MessageSquare size={22} aria-hidden /> },
  { id: "missions", label: "Missions: Autopilot and Auto Improve", icon: <Rocket size={22} aria-hidden /> },
  { id: "network", label: "Gateways", icon: <Network size={22} aria-hidden /> },
  { id: "extensions", label: "Extensions", icon: <Blocks size={22} aria-hidden /> },
];

export function App() {
  const [ws, setWs] = useState<WorkspaceInfo | null>(null);
  const [view, setView] = useState<View>("explorer");
  const [activeRel, setActiveRel] = useState<string | null>(null);
  const [cursor, setCursor] = useState({ line: 1, col: 1, language: "plaintext" });
  const [notice, setNotice] = useState("");
  // Bumped on every folder open, so the chat starts fresh even when two folders share a name.
  const [wsGen, setWsGen] = useState(0);
  const [gateways, setGateways] = useState<GatewayList>({ gateways: [], active: null });
  const editor = useRef<EditorHandle>(null);
  // The assistant's change being reviewed in the diff view, if any.
  const [review, setReview] = useState<Review | null>(null);
  const onReview = useCallback((r: Review | null) => setReview(r), []);
  const onFileWritten = useCallback((rel: string) => editor.current?.fileWritten(rel), []);

  // After a webview reload the Rust side may still have a workspace open; saved gateways come from the core too.
  useEffect(() => {
    workspaceInfo().then(setWs).catch(() => undefined);
    gatewaysList().then(setGateways).catch(() => undefined);
  }, []);
  const activeGateway = gateways.gateways.find((g) => g.id === gateways.active) ?? null;

  const onNotice = useCallback((m: string) => setNotice(m), []);
  const onCursor = useCallback((line: number, col: number, language: string) => setCursor({ line, col, language }), []);
  const onActive = useCallback((rel: string | null) => setActiveRel(rel), []);

  const openFolder = async () => {
    try {
      const info = await pickFolder();
      if (info) {
        editor.current?.reset();
        setReview(null);
        setWs(info);
        setWsGen((g) => g + 1);
        setView("explorer");
        setNotice(`Opened ${info.name}`);
      }
    } catch (e) {
      setNotice((e as Error).message);
    }
  };

  const side = () => {
    switch (view) {
      case "explorer":
        // The tree itself stays mounted below (so expanded folders survive switching views); this is the empty state.
        return ws ? null : (
          <div className="flex flex-col gap-3 px-3">
            <p className="text-ide-dim">No folder is open.</p>
            <button type="button" onClick={() => void openFolder()} className="rounded bg-ide-accent px-3 py-1.5 text-white">Open folder</button>
          </div>
        );
      case "search":
        return ws ? <SearchView onOpenAt={(rel, line) => void editor.current?.open(rel, line)} onError={onNotice} /> : <p className="px-3 text-ide-dim">Open a folder to search it.</p>;
      case "chat":
        // ChatView stays mounted below (so a chat in progress and its pending changes survive switching views).
        return null;
      case "missions":
        return <Placeholder title="Missions" phase="phase I4">Autopilot and Auto Improve run on the gateway on a synced copy of this folder. Auto Train stays with the gateway admin.</Placeholder>;
      case "network":
        return <NetworkView onChanged={setGateways} onNotice={onNotice} />;
      case "extensions":
        return <Placeholder title="Extensions" phase="phase I6">VS Code and VSCodium extensions from Open VSX, in folders you trust.</Placeholder>;
    }
  };

  return (
    <Tooltip.Provider>
      <div className="flex h-full flex-col">
        <header className="flex h-[35px] shrink-0 items-center gap-3 border-b border-ide-line bg-ide-side px-3">
          <span className="font-semibold text-ide-bright">P2P-AI IDE</span>
          <span className="flex-1 truncate text-center text-ide-dim">{ws ? ws.name : "No folder open"}</span>
          <button type="button" onClick={() => void openFolder()} className="flex items-center gap-1 rounded px-2 py-1 hover:bg-ide-hover">
            <FolderOpen size={15} aria-hidden />Open folder
          </button>
        </header>
        <div className="flex min-h-0 flex-1">
          <nav aria-label="Views" className="flex w-12 shrink-0 flex-col items-center border-r border-ide-line bg-ide-bar pt-1">
            {VIEWS.map((v) => (
              <Tip key={v.id} text={v.label} side="right">
                <button
                  type="button"
                  aria-label={v.label}
                  aria-pressed={view === v.id}
                  onClick={() => setView(v.id)}
                  className={`flex h-12 w-12 items-center justify-center ${view === v.id ? "text-white shadow-[inset_2px_0_0_var(--color-ide-focus)]" : "text-[#858585] hover:text-ide-bright"}`}
                >
                  {v.icon}
                </button>
              </Tip>
            ))}
          </nav>
          <aside aria-label="Side bar" className={`flex ${view === "chat" || view === "network" ? "w-96" : "w-64"} shrink-0 flex-col border-r border-ide-line bg-ide-side`}>
            <div className="px-4 py-2 text-[11px] uppercase tracking-wide text-ide-dim">{VIEWS.find((v) => v.id === view)?.label}</div>
            {ws ? (
              <div className={view === "explorer" ? "flex min-h-0 flex-1 flex-col" : "hidden"}>
                <Explorer workspaceName={ws.name} activeRel={activeRel} onOpenFile={(rel) => void editor.current?.open(rel)} onError={onNotice} />
              </div>
            ) : null}
            <div className={view === "chat" ? "flex min-h-0 flex-1 flex-col" : "hidden"}>
              <ChatView gateway={activeGateway} workspaceKey={String(wsGen)} onNotice={onNotice} onOpenGateways={() => setView("network")} hasFolder={ws !== null} onReview={onReview} onFileWritten={onFileWritten} />
            </div>
            {side()}
          </aside>
          <div className="relative flex min-h-0 min-w-0 flex-1">
            <Editor ref={editor} onActiveChange={onActive} onCursor={onCursor} onNotice={onNotice} />
            {review ? <DiffReview change={review.change} onAccept={review.accept} onReject={review.reject} onClose={() => setReview(null)} /> : null}
          </div>
        </div>
        <StatusBar line={cursor.line} col={cursor.col} language={cursor.language} notice={notice} gateway={activeGateway} onGateway={() => setView("network")} />
      </div>
    </Tooltip.Provider>
  );
}
