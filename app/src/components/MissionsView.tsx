/**
 * MissionsView.tsx -- the Missions side bar: Autopilot and Auto Improve on the gateway (phase I4,
 * docs/features/missions.md).
 *
 * Purpose: missions run on a Go gateway, on a project that holds a copy of the open folder. Here the person links
 * the folder to a project, sends the folder (secrets stay home), switches Autopilot and Auto Improve on for the
 * project, starts a mission with a goal (and, for Autopilot, the checks that say when it is done), watches it,
 * approves or rejects the write calls it asks for, triages Auto Improve's proposals, and brings the finished work
 * back into "Changes to review", where each file is accepted or rejected in the diff view as in Agent chat.
 * Trust boundary: everything shown here comes from the gateway (untrusted). It is cleaned in Rust and rendered with
 * React text nodes only; approval arguments are shown verbatim in a <pre>; proposal sources are plain text, never
 * links (CWE-79, OWASP LLM01). Nothing a mission or proposal says is run by the IDE.
 */
import { useCallback, useEffect, useState } from "react";
import { Check, Download, Link2, Pause, Play, RefreshCw, Rocket, Send, Sparkles, Square, Unlink, X } from "lucide-react";
import {
  approvalDecide, approvalsList, gatewayCheck, missionControl, missionProject, missionStart, missionsList, projectCollect,
  projectCreate, projectLink, projectPull, projectPush, projectsList, proposalAct, proposalsList, switchesSet,
  type ApprovalView, type CheckInput, type GatewayView, type MissionView, type ProjectState, type ProjectView,
  type ProposalView, type PushReport, type Switches,
} from "../ipc.ts";
import { Tip } from "./Tip.tsx";

interface Props {
  gateway: GatewayView | null;
  folderName: string | null;
  onNotice: (m: string) => void;
  onOpenGateways: () => void;
  /** Files were added to "Changes to review"; the parent refreshes the list and may open Agent chat. */
  onPulled: (count: number) => void;
  /** The linked project's switches (or null), for the status bar. */
  onSwitches: (s: Switches | null) => void;
}

interface CheckRow { id: string; kind: "tests" | "command"; framework: string; command: string }

const FRAMEWORKS = ["pytest", "unittest", "npm", "cargo", "go", "make"];
const btn = "flex items-center gap-1 rounded border border-ide-line px-2 py-1 hover:bg-ide-hover disabled:opacity-50";
const primary = "flex items-center gap-1 rounded bg-ide-accent px-2 py-1 text-white disabled:opacity-50";
const section = "flex flex-col gap-2 border-t border-ide-line px-3 py-2";
const head = "text-[11px] uppercase tracking-wide text-ide-dim";

/** Turns a command line typed by the person into words (split on spaces; no shell, no quoting rules). */
const words = (s: string) => s.split(" ").map((w) => w.trim()).filter(Boolean);

export function MissionsView({ gateway, folderName, onNotice, onOpenGateways, onPulled, onSwitches }: Props) {
  const [st, setSt] = useState<ProjectState | null>(null);
  const [projects, setProjects] = useState<ProjectView[] | null>(null);
  const [newName, setNewName] = useState(folderName ?? "");
  const [busy, setBusy] = useState("");
  const [push, setPush] = useState<PushReport | null>(null);
  const [missions, setMissions] = useState<MissionView[]>([]);
  const [proposals, setProposals] = useState<ProposalView[]>([]);
  const [approvals, setApprovals] = useState<Record<string, ApprovalView[]>>({});
  const [models, setModels] = useState<string[]>([]);
  const [kind, setKind] = useState<"autopilot" | "improve">("autopilot");
  const [goal, setGoal] = useState("");
  const [model, setModel] = useState("");
  const [checks, setChecks] = useState<CheckRow[]>([{ id: "tests", kind: "tests", framework: "pytest", command: "" }]);
  const [stopArmed, setStopArmed] = useState<string | null>(null);

  const fail = useCallback((e: unknown) => onNotice((e as Error).message), [onNotice]);

  const hasCreds = Boolean(gateway && (gateway.has_key || gateway.account !== null));
  const supportsMissions = Boolean(gateway && hasCreds && gateway.kind !== "tandemforge" && gateway.kind !== "nexus");
  const refresh = useCallback(async () => {
    if (!supportsMissions || !folderName) return;
    try {
      const s = await missionProject();
      setSt(s);
      onSwitches(s.switches);
      if (s.project_id) {
        setMissions(await missionsList().catch(() => []));
        setProposals(await proposalsList().catch(() => []));
      }
    } catch (e) {
      fail(e);
    }
  }, [supportsMissions, folderName, fail, onSwitches]);

  useEffect(() => {
    setSt(null);
    setProjects(null);
    setPush(null);
    setMissions([]);
    setProposals([]);
    setNewName(folderName ?? "");
    void refresh();
  }, [gateway?.id, folderName, refresh]);

  useEffect(() => {
    setModels([]);
    if (!supportsMissions || !gateway) return;
    gatewayCheck(gateway.id).then((r) => {
      setModels(r.models);
      setModel((m) => (r.models.includes(m) ? m : (r.models[0] ?? "")));
    }).catch(() => undefined);
  }, [gateway?.id, supportsMissions]);

  // While a mission runs, refresh its state every 10 seconds.
  const running = missions.some((m) => m.state === "running");
  useEffect(() => {
    if (!running) return;
    const t = setInterval(() => void refresh(), 10_000);
    return () => clearInterval(t);
  }, [running, refresh]);

  const run = async (label: string, f: () => Promise<void>) => {
    setBusy(label);
    try { await f(); } catch (e) { fail(e); } finally { setBusy(""); }
  };

  if (!folderName) return <p className="px-3 text-ide-dim">Open a folder first. Missions work on a copy of the open folder on a gateway.</p>;
  if (!gateway) {
    return (
      <div className="flex flex-col gap-2 px-3">
        <p className="text-ide-dim">Missions run on a gateway. Add one first.</p>
        <button type="button" className={primary} onClick={onOpenGateways}>Open Gateways</button>
      </div>
    );
  }
  if (gateway.kind === "tandemforge" || gateway.kind === "nexus") {
    return (
      <div className="flex flex-col gap-3 px-3">
        <p className="text-ide-dim">Missions run on a P2P-AI gateway project. {gateway.name} is a local model engine on this computer — use Agent chat for local work, or switch to a remote gateway.</p>
        <button type="button" onClick={onOpenGateways} className="rounded bg-ide-accent px-3 py-1.5 text-white">Open Gateways</button>
      </div>
    );
  }
  if (!hasCreds) return <p className="px-3 text-ide-dim">Add a key (or link an account) for {gateway.name} in the Gateways view first.</p>;
  if (!st) return <p className="px-3 text-ide-dim">Loading…</p>;

  // ---- not linked yet: pick or create a project
  if (!st.project_id) {
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-3 pb-3">
        <p className="text-ide-dim">Link this folder to a project on {st.gateway_name}. The folder is sent there for missions; files that may hold secrets (.env, keys, .git) stay on this computer.</p>
        <label className="flex flex-col gap-1">New project name
          <input aria-label="New project name" value={newName} onChange={(e) => setNewName(e.target.value)} className="rounded border border-ide-line bg-ide-bg px-2 py-1" />
        </label>
        <button type="button" className={primary} disabled={!!busy || !newName.trim()} onClick={() => void run("create", async () => {
          const p = await projectCreate(newName.trim());
          onNotice(`Created project ${p.name}`);
          await refresh();
        })}>Create project and link</button>
        <button type="button" className={btn} disabled={!!busy} onClick={() => void run("list", async () => setProjects(await projectsList()))}>
          <Link2 size={14} aria-hidden />Link an existing project…
        </button>
        {projects ? (
          <ul aria-label="Gateway projects" className="flex flex-col gap-1">
            {projects.length === 0 ? <li className="text-ide-dim">No projects for this key yet.</li> : null}
            {projects.map((p) => (
              <li key={p.id} className="flex items-center gap-2">
                <span className="flex-1 truncate" title={p.name}>{p.name}</span>
                <span className="text-xs text-ide-dim">{p.role} · {p.file_count} files</span>
                <button type="button" className={btn} onClick={() => void run("link", async () => { await projectLink(p.id, p.name); await refresh(); })}>Link</button>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    );
  }

  const sw = st.switches;
  const copyReady = st.sync?.state === "ready";
  const setSwitch = (autopilot: boolean, autoImprove: boolean) =>
    void run("switch", async () => { const s = await switchesSet(autopilot, autoImprove); setSt({ ...st, switches: s }); onSwitches(s); });

  const start = () => void run("start", async () => {
    const list: CheckInput[] = kind === "autopilot"
      ? checks.map((c) => (c.kind === "tests" ? { id: c.id, kind: "tests", framework: c.framework } : { id: c.id, kind: "command", argv: words(c.command) }))
      : [];
    const m = await missionStart(kind, goal, model, list);
    onNotice(`${m.kind === "autopilot" ? "Autopilot" : "Auto Improve"} started`);
    setGoal("");
    await refresh();
  });

  const showApprovals = (m: MissionView) => void run("approvals", async () => {
    const a = await approvalsList(m.run_id);
    setApprovals((x) => ({ ...x, [m.id]: a }));
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto pb-3">
      <div className="flex flex-col gap-1 px-3 pb-2">
        <div className="flex items-center gap-2">
          <span className="flex-1 truncate text-ide-bright" title={st.project_name}>{st.project_name}</span>
          <Tip text="Refresh"><button type="button" aria-label="Refresh missions" className="rounded p-1 hover:bg-ide-hover" onClick={() => void refresh()}><RefreshCw size={14} aria-hidden /></button></Tip>
          <Tip text="Unlink this folder from the project (nothing is deleted on the gateway)">
            <button type="button" aria-label="Unlink project" className="rounded p-1 hover:bg-ide-hover" onClick={() => void run("unlink", async () => { await projectLink(null, ""); await refresh(); })}><Unlink size={14} aria-hidden /></button>
          </Tip>
        </div>
        <span className="text-xs text-ide-dim">on {st.gateway_name} · copy on a node: {st.sync?.state || "none"}</span>
        {st.note ? <span className="text-xs text-[#f48771]">{st.note}</span> : null}
        <button type="button" className={primary} disabled={!!busy} onClick={() => void run("push", async () => {
          const r = await projectPush();
          setPush(r);
          onNotice(`Sent ${r.uploaded} files, removed ${r.deleted}, ${r.unchanged} unchanged`);
          await refresh();
        })}><Send size={14} aria-hidden />{busy === "push" ? "Sending…" : "Send folder to the project"}</button>
        {push ? (
          <div aria-label="Last send" className="text-xs text-ide-dim">
            Version {push.version}: {push.uploaded} sent, {push.deleted} removed, {push.unchanged} unchanged. Copy: {push.copy || "none"}.
            {push.skipped.length ? (
              <details><summary>{push.skipped.length} kept on this computer</summary>
                <ul>{push.skipped.slice(0, 100).map((s) => <li key={s.rel}>{s.rel}: {s.reason}</li>)}</ul>
              </details>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className={section}>
        <span className={head}>Switches for this project</span>
        {sw?.disabled ? <span className="text-xs text-[#f48771]">The gateway's administrator has switched missions off.</span> : null}
        <Tip text={sw?.autopilot_available ? "Autopilot works on a goal until your checks pass. Every file change waits for your approval." : "This gateway does not offer Autopilot."}>
          <label className="flex items-center gap-2"><input type="checkbox" aria-label="Autopilot" disabled={!sw?.autopilot_available || !!busy} checked={!!sw?.autopilot} onChange={(e) => setSwitch(e.target.checked, !!sw?.auto_improve)} /><Rocket size={14} aria-hidden />Autopilot</label>
        </Tip>
        <Tip text={sw?.improve_available ? "Auto Improve suggests ideas from a summary of the project. It never changes files." : "This gateway does not offer Auto Improve."}>
          <label className="flex items-center gap-2"><input type="checkbox" aria-label="Auto Improve" disabled={!sw?.improve_available || !!busy} checked={!!sw?.auto_improve} onChange={(e) => setSwitch(!!sw?.autopilot, e.target.checked)} /><Sparkles size={14} aria-hidden />Auto Improve</label>
        </Tip>
      </div>

      <div className={section}>
        <span className={head}>Start a mission</span>
        <div className="flex gap-3">
          <label className="flex items-center gap-1"><input type="radio" name="kind" checked={kind === "autopilot"} onChange={() => setKind("autopilot")} />Autopilot</label>
          <label className="flex items-center gap-1"><input type="radio" name="kind" checked={kind === "improve"} onChange={() => setKind("improve")} />Auto Improve</label>
        </div>
        <textarea aria-label="Mission goal" value={goal} onChange={(e) => setGoal(e.target.value)} rows={3} placeholder={kind === "autopilot" ? "What should be done? e.g. Make the tests pass" : "What should it look for? e.g. Integrations for CI systems"} className="rounded border border-ide-line bg-ide-bg px-2 py-1" />
        <select aria-label="Mission model" value={model} onChange={(e) => setModel(e.target.value)} className="rounded border border-ide-line bg-ide-bg px-1 py-1">
          {models.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
        {kind === "autopilot" ? (
          <div className="flex flex-col gap-1" aria-label="Done when">
            <span className="text-xs text-ide-dim">Done when these checks pass (run by the gateway, not the model):</span>
            {checks.map((c, i) => (
              <div key={i} className="flex items-center gap-1">
                <input aria-label="Check name" value={c.id} onChange={(e) => setChecks(checks.map((x, j) => (j === i ? { ...x, id: e.target.value } : x)))} className="w-16 rounded border border-ide-line bg-ide-bg px-1" />
                <select aria-label="Check type" value={c.kind} onChange={(e) => setChecks(checks.map((x, j) => (j === i ? { ...x, kind: e.target.value as CheckRow["kind"] } : x)))} className="rounded border border-ide-line bg-ide-bg">
                  <option value="tests">tests</option><option value="command">command</option>
                </select>
                {c.kind === "tests" ? (
                  <select aria-label="Test framework" value={c.framework} onChange={(e) => setChecks(checks.map((x, j) => (j === i ? { ...x, framework: e.target.value } : x)))} className="flex-1 rounded border border-ide-line bg-ide-bg">
                    {FRAMEWORKS.map((f) => <option key={f} value={f}>{f}</option>)}
                  </select>
                ) : (
                  <input aria-label="Check command" placeholder="ruff check ." value={c.command} onChange={(e) => setChecks(checks.map((x, j) => (j === i ? { ...x, command: e.target.value } : x)))} className="min-w-0 flex-1 rounded border border-ide-line bg-ide-bg px-1" />
                )}
                <button type="button" aria-label="Remove check" className="rounded p-0.5 hover:bg-ide-hover" disabled={checks.length === 1} onClick={() => setChecks(checks.filter((_, j) => j !== i))}><X size={12} aria-hidden /></button>
              </div>
            ))}
            {checks.length < 8 ? <button type="button" className="self-start text-xs underline" onClick={() => setChecks([...checks, { id: `check${checks.length + 1}`, kind: "command", framework: "pytest", command: "" }])}>Add a check</button> : null}
            {!copyReady ? <span className="text-xs text-ide-dim">Autopilot works on the node copy; send the folder first and wait until the copy is ready.</span> : null}
          </div>
        ) : null}
        <button type="button" className={primary} disabled={!!busy || goal.trim().length < 3 || !model || (kind === "autopilot" ? !sw?.autopilot : !sw?.auto_improve)} onClick={start}>
          <Play size={14} aria-hidden />Start {kind === "autopilot" ? "Autopilot" : "Auto Improve"}
        </button>
      </div>

      <div className={section}>
        <span className={head}>Missions</span>
        {missions.length === 0 ? <span className="text-ide-dim">No missions on this project yet.</span> : null}
        <ul aria-label="Missions" className="flex flex-col gap-2">
          {missions.map((m) => (
            <li key={m.id} className="flex flex-col gap-1 rounded border border-ide-line p-2">
              <div className="flex items-center gap-1">
                {m.kind === "autopilot" ? <Rocket size={13} aria-hidden /> : <Sparkles size={13} aria-hidden />}
                <span className="flex-1 truncate" title={m.goal}>{m.goal}</span>
                <span className="text-xs">{m.state}</span>
              </div>
              <span className="text-xs text-ide-dim">
                cycle {m.cycle}/{m.max_cycles} · {m.tokens} tokens
                {m.kind === "autopilot" ? ` · checks ${m.checks_passed}/${m.checks_total} · tasks ${m.tasks_done}/${m.tasks_total}` : ` · ${m.proposals_created} ideas`}
              </span>
              {m.reason ? <span className="text-xs text-ide-dim">{m.reason}</span> : null}
              <div className="flex flex-wrap gap-1">
                {m.state === "running" ? <button type="button" className={btn} onClick={() => void run("ctl", async () => { await missionControl(m.id, "pause"); await refresh(); })}><Pause size={12} aria-hidden />Pause</button> : null}
                {m.state === "paused" || m.state === "stalled" ? <button type="button" className={btn} onClick={() => void run("ctl", async () => { await missionControl(m.id, "resume"); await refresh(); })}><Play size={12} aria-hidden />Resume</button> : null}
                {["running", "paused", "stalled"].includes(m.state) ? (
                  stopArmed === m.id
                    ? <button type="button" className={btn} onClick={() => void run("ctl", async () => { setStopArmed(null); await missionControl(m.id, "stop"); await refresh(); })}><Square size={12} aria-hidden />Stop for good</button>
                    : <button type="button" className={btn} onClick={() => setStopArmed(m.id)}><Square size={12} aria-hidden />Stop</button>
                ) : null}
                {m.waiting_approval && m.run_id ? <button type="button" className={primary} onClick={() => showApprovals(m)}>Review approval</button> : null}
              </div>
              {(approvals[m.id] ?? []).map((a) => (
                <div key={a.id} aria-label={`Approval ${a.tool}`} className="flex flex-col gap-1 rounded bg-ide-bg p-1">
                  <span className="text-xs">The mission wants to run <b>{a.tool}</b> ({a.access}):</span>
                  <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-words font-mono text-xs">{a.arguments}</pre>
                  <div className="flex gap-1">
                    <button type="button" className={primary} onClick={() => void run("decide", async () => { await approvalDecide(m.run_id, a.id, "approve", a.arguments_sha256); setApprovals((x) => ({ ...x, [m.id]: [] })); await refresh(); })}><Check size={12} aria-hidden />Approve</button>
                    <button type="button" className={btn} onClick={() => void run("decide", async () => { await approvalDecide(m.run_id, a.id, "reject", a.arguments_sha256); setApprovals((x) => ({ ...x, [m.id]: [] })); await refresh(); })}><X size={12} aria-hidden />Reject</button>
                  </div>
                </div>
              ))}
              {m.tasks.length ? (
                <details><summary className="text-xs">Tasks</summary>
                  <ul className="text-xs">{m.tasks.map((t, i) => <li key={i}>{t.state}: {t.title}</li>)}</ul>
                </details>
              ) : null}
            </li>
          ))}
        </ul>
      </div>

      {proposals.length ? (
        <div className={section}>
          <span className={head}>Auto Improve ideas</span>
          <ul aria-label="Proposals" className="flex flex-col gap-2">
            {proposals.map((p) => (
              <li key={p.id} className="flex flex-col gap-1 rounded border border-ide-line p-2">
                <span className="text-ide-bright">{p.title}</span>
                <span className="text-xs text-ide-dim">{p.kind} · effort {p.effort} · {p.state}</span>
                <span className="whitespace-pre-wrap text-xs">{p.rationale}</span>
                {p.sources.length ? <span className="break-all text-xs text-ide-dim">Sources (not opened): {p.sources.join(", ")}</span> : null}
                {p.state === "new" ? (
                  <div className="flex gap-1">
                    <button type="button" className={btn} onClick={() => void run("prop", async () => { await proposalAct(p.id, "accept"); await refresh(); })}>Accept</button>
                    <button type="button" className={btn} onClick={() => void run("prop", async () => { await proposalAct(p.id, "dismiss"); await refresh(); })}>Dismiss</button>
                    <button type="button" className={btn} onClick={() => void run("prop", async () => { await proposalAct(p.id, "snooze"); await refresh(); })}>Snooze a week</button>
                  </div>
                ) : null}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      <div className={section}>
        <span className={head}>Bring the work back</span>
        <span className="text-xs text-ide-dim">1. Collect the node copy's approved changes into the project. 2. Review them here file by file; nothing is written until you accept.</span>
        {st.sync?.delta_state ? (
          <span aria-label="Last collect" className="text-xs">
            Last collect: {st.sync.delta_state}
            {st.sync.delta_state === "committed" ? ` (version ${st.sync.delta_version}: ${st.sync.delta_added} added, ${st.sync.delta_modified} changed, ${st.sync.delta_deleted} deleted)` : ""}
            {st.sync.delta_conflicts.length ? `; changed on both sides: ${st.sync.delta_conflicts.join(", ")}` : ""}
          </span>
        ) : null}
        <div className="flex gap-1">
          <button type="button" className={btn} disabled={!!busy || !copyReady} onClick={() => void run("collect", async () => {
            const s = await projectCollect();
            setSt({ ...st, sync: s });
          })}>Collect changes</button>
          <button type="button" className={primary} disabled={!!busy} onClick={() => void run("pull", async () => {
            const r = await projectPull();
            onNotice(r.proposed ? `${r.proposed} files wait in Changes to review` : "No changed files to review");
            if (r.skipped.length) onNotice(`${r.proposed} files to review; ${r.skipped.length} left out (${r.skipped[0].rel}: ${r.skipped[0].reason})`);
            onPulled(r.proposed);
          })}><Download size={14} aria-hidden />Review changes</button>
        </div>
      </div>
    </div>
  );
}
