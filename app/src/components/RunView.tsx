/**
 * RunView.tsx -- the Run and test view (phase I5b, docs/features/run-and-test.md): run the project's tests with one
 * framework, or one program with its arguments, in a sandbox, and read the result (exit code, test counts, failing
 * tests, output).
 *
 * Runners: the active gateway's sandbox (I5b; the folder is sent to the linked project first, as in Missions) and
 * the local OS sandbox on this computer (I5c; a copy of the folder, no network, Landlock and seccomp on Linux, an
 * AppContainer on Windows), and a local Docker or Podman container (I5d; images from a fixed, digest-pinned list,
 * downloaded only when the person presses Get image). The runner and the command image are shared with the chat.
 *
 * Trust boundary: everything a run prints is untrusted program output. It is rendered by React as text in <pre>
 * blocks, never as HTML or links (CWE-79). The command is split into words here and checked again in the Rust core;
 * there is no shell, so `;`, `|` or `$()` are just text for the program (CWE-78).
 * Dependencies: React, lucide-react icons.
 */
import { useEffect, useState } from "react";
import { Play } from "lucide-react";
import { containerImages, containerPrepare, FRAMEWORKS, imageFor, runnersList, runStart, splitWords, type ImageView, type RunnerView, type RunResult } from "../ipc.ts";

interface Props {
  /** The runner in use (shared with the chat's runs). */
  runner: string;
  onRunner: (r: string) => void;
  /** The image for commands on the container runner (shared with the chat's runs). */
  image: string;
  onImage: (i: string) => void;
  onNotice: (m: string) => void;
  /** Opens the Missions view, where the folder is linked to a gateway project. */
  onOpenMissions: () => void;
  /** True while the view is on screen. The view stays mounted when hidden, so the runner list is read again
   *  each time it is shown (a gateway added or a folder linked meanwhile must not leave a stale reason). */
  visible: boolean;
  /** The active gateway's id (or null): a change re-reads the runner list. */
  gatewayId: string | null;
}

/** Words for the stages a gateway run goes through. */
const STAGES: Record<string, string> = {
  copying_local: "Copying the folder into the sandbox…",
  sending: "Sending changed files to the project…",
  copying: "Waiting for the node's copy to be up to date…",
  running: "Running in the sandbox…",
  pulling: "Downloading the image…",
  building: "Building the image (this downloads packages and can take minutes)…",
};

/** Labels for the runners. */
const LABELS: Record<string, string> = {
  gateway: "Gateway sandbox (no network, changes not kept)",
  "local-os": "Local sandbox on this computer (no network, changes not kept)",
  container: "Local container, Docker or Podman",
};

export function RunView({ runner, onRunner: setRunner, image, onImage: setImage, onNotice, onOpenMissions, visible, gatewayId }: Props) {
  const [runners, setRunners] = useState<RunnerView[]>([]);
  // Which runners work now (the gateway needs a linked folder; the local sandbox needs Landlock and namespaces).
  const refresh = () => { runnersList().then(setRunners).catch((e) => onNotice((e as Error).message)); };
  // Read again when the view is shown or the active gateway changes, not only at mount (click-through 2026-11-23:
  // the gateway option kept "add a gateway in the Gateways view first" after a gateway was saved).
  useEffect(() => { if (visible) refresh(); }, [onNotice, visible, gatewayId]);
  const [kind, setKind] = useState<"tests" | "command">("tests");
  const [framework, setFramework] = useState<string>("pytest");
  const [line, setLine] = useState("");
  const [cwd, setCwd] = useState("");
  const [timeout, setTimeoutS] = useState(120);
  const [stage, setStage] = useState("");
  const [result, setResult] = useState<RunResult | null>(null);
  const [error, setError] = useState("");

  // Container images (phase I5d): which are on this computer, so a missing one offers Get image instead of a run.
  const [images, setImages] = useState<ImageView[]>([]);
  const loadImages = () => { containerImages().then(setImages).catch((e) => onNotice((e as Error).message)); };
  useEffect(() => { if (runner === "container") loadImages(); }, [runner]);
  const needed = imageFor(kind, framework, image);
  const neededView = images.find((i) => i.id === needed);
  const missing = runner === "container" && neededView !== undefined && !neededView.present;

  const argv = splitWords(line);
  const ready = stage === "" && !missing && (kind === "tests" || argv.length > 0);

  const getImage = async () => {
    setError("");
    setStage("starting");
    try {
      await containerPrepare(needed, setStage);
      onNotice(`${neededView?.label ?? needed} is ready`);
    } catch (e) {
      const m = (e as Error).message;
      setError(m);
      onNotice(m);
    } finally {
      setStage("");
      loadImages();
    }
  };

  const start = async () => {
    setResult(null);
    setError("");
    setStage("starting");
    try {
      const input = kind === "tests"
        ? { kind, framework, cwd, timeout_s: timeout }
        : { kind, argv, cwd, timeout_s: timeout };
      setResult(await runStart(runner, input, (st) => setStage(runner !== "gateway" && st === "copying" ? "copying_local" : st), image));
    } catch (e) {
      const m = (e as Error).message;
      setError(m);
      onNotice(m);
    } finally {
      setStage("");
    }
  };

  const verdict = (r: RunResult) =>
    r.error ? `Could not run: ${r.error}` : r.timed_out ? "Stopped: the time limit was reached" : r.exit_code === 0 ? "Passed (exit code 0)" : `Failed (exit code ${r.exit_code ?? "none"})`;

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto px-3 pb-4">
      <label htmlFor="run-runner" className="text-xs text-ide-dim">Where to run</label>
      <select id="run-runner" value={runner} onFocus={refresh} onChange={(e) => setRunner(e.target.value)} className="rounded border border-ide-line bg-ide-bg px-1 py-1 text-ide-bright">
        {(runners.length ? runners : [{ id: "gateway", available: true, reason: "" }]).map((r) => (
          <option key={r.id} value={r.id} disabled={!r.available && r.id !== runner}>
            {LABELS[r.id] ?? r.id}{r.available ? "" : ` (${r.reason})`}
          </option>
        ))}
      </select>
      {runner === "container" ? (
        <p className="text-xs text-ide-dim">
          Runs on a copy of the folder in a throw-away container: no network, no capabilities, not root, a read-only image, and its changes are not kept. Secrets such as .env and keys are not copied. Images are Red Hat UBI 9, pinned by digest.
        </p>
      ) : runner === "local-os" ? (
        <p className="text-xs text-ide-dim">
          Runs on a copy of the folder on this computer: no network, your home folder hidden except toolchains (read only), and its changes are not kept. Secrets such as .env and keys are not copied.
        </p>
      ) : (
        <p className="text-xs text-ide-dim">
          The gateway sandbox runs on a node of the linked project, on the files as they are on disk. Secrets such as .env and keys are never sent.{" "}
          <button type="button" onClick={onOpenMissions} className="underline">Link the folder in Missions</button>
        </p>
      )}

      <fieldset className="flex gap-3" aria-label="What to run">
        <label className="flex items-center gap-1"><input type="radio" name="run-kind" checked={kind === "tests"} onChange={() => setKind("tests")} />Tests</label>
        <label className="flex items-center gap-1"><input type="radio" name="run-kind" checked={kind === "command"} onChange={() => setKind("command")} />Command</label>
      </fieldset>
      {kind === "tests" ? (
        <>
          <label htmlFor="run-fw" className="text-xs text-ide-dim">Test framework</label>
          <select id="run-fw" value={framework} onChange={(e) => setFramework(e.target.value)} className="rounded border border-ide-line bg-ide-bg px-1 py-1 text-ide-bright">
            {FRAMEWORKS.map((f) => <option key={f} value={f}>{f}</option>)}
          </select>
        </>
      ) : (
        <>
          <label htmlFor="run-cmd" className="text-xs text-ide-dim">Program and arguments (no shell; "quotes" keep spaces)</label>
          <input id="run-cmd" value={line} maxLength={8000} onChange={(e) => setLine(e.target.value)} placeholder="python3 -m pytest -q"
            className="rounded border border-ide-line bg-ide-bg px-2 py-1 font-mono text-ide-bright" />
          {runner === "container" ? (
            <>
              <label htmlFor="run-image" className="text-xs text-ide-dim">Image</label>
              <select id="run-image" value={image} onChange={(e) => setImage(e.target.value)} className="rounded border border-ide-line bg-ide-bg px-1 py-1 text-ide-bright">
                {(images.length ? images : [{ id: "python", label: "Python 3.12 (UBI 9)", present: true, get: "pull" as const }]).map((i) => (
                  <option key={i.id} value={i.id}>{i.label}{i.present ? "" : " (not on this computer yet)"}</option>
                ))}
              </select>
            </>
          ) : null}
        </>
      )}
      {missing && neededView ? (
        <div className="flex flex-col gap-1 rounded border border-ide-line p-2 text-xs">
          <p>{neededView.label} is not on this computer yet. {neededView.get === "build" ? "Building it downloads the UBI 9 base and Rust packages." : "Getting it downloads the pinned image."}</p>
          <button type="button" disabled={stage !== ""} onClick={() => void getImage()} className="self-start rounded border border-ide-line px-2 py-1 text-ide-bright disabled:opacity-50">
            Get image
          </button>
        </div>
      ) : null}
      <div className="flex gap-2">
        <label className="flex min-w-0 flex-1 flex-col text-xs text-ide-dim">Folder (from the root)
          <input aria-label="Folder to run in" value={cwd} maxLength={1000} onChange={(e) => setCwd(e.target.value)} placeholder="(root)" className="rounded border border-ide-line bg-ide-bg px-2 py-1 text-sm text-ide-bright" />
        </label>
        <label className="flex w-24 flex-col text-xs text-ide-dim">Time limit (s)
          <input aria-label="Time limit in seconds" type="number" min={10} max={300} value={timeout} onChange={(e) => setTimeoutS(Number(e.target.value) || 120)} className="rounded border border-ide-line bg-ide-bg px-2 py-1 text-sm text-ide-bright" />
        </label>
      </div>
      <button type="button" disabled={!ready} onClick={() => void start()} className="flex items-center justify-center gap-1 rounded bg-ide-accent px-3 py-1 text-white disabled:opacity-50">
        <Play size={14} aria-hidden />{kind === "tests" ? `Run ${framework} tests` : "Run command"}
      </button>
      {stage ? <p role="status" className="text-xs text-ide-dim">{STAGES[stage] ?? "Starting…"}</p> : null}
      {error ? <p role="alert" className="text-xs text-[#f48771]">{error}</p> : null}

      {result ? (
        <section aria-label="Run result" className="flex flex-col gap-1">
          <p role="status" className={result.exit_code === 0 && !result.timed_out && !result.error ? "text-[#89d185]" : "text-[#f48771]"}>{verdict(result)}</p>
          <p className="text-xs text-ide-dim">{result.summary}{result.place ? ` · on ${result.place}` : ""} · {(result.duration_ms / 1000).toFixed(1)} s</p>
          {result.tests ? (
            <p className="text-xs text-ide-bright">Tests: {result.tests.passed} passed, {result.tests.failed} failed, {result.tests.skipped} skipped</p>
          ) : null}
          {result.tests?.failures.length ? (
            <ul aria-label="Failing tests" className="text-xs text-[#f48771]">
              {result.tests.failures.map((f) => <li key={f} className="truncate font-mono" title={f}>{f}</li>)}
            </ul>
          ) : null}
          {result.truncated ? <p className="text-xs text-ide-dim">The output was cut to its limit.</p> : null}
          <div className="text-xs text-ide-dim">Output</div>
          <pre aria-label="Standard output" className="max-h-72 overflow-auto whitespace-pre-wrap break-words rounded bg-ide-bg p-2 font-mono text-xs">{result.stdout || "(nothing)"}</pre>
          {result.stderr ? (
            <>
              <div className="text-xs text-ide-dim">Errors</div>
              <pre aria-label="Error output" className="max-h-48 overflow-auto whitespace-pre-wrap break-words rounded bg-ide-bg p-2 font-mono text-xs">{result.stderr}</pre>
            </>
          ) : null}
        </section>
      ) : null}
    </div>
  );
}
