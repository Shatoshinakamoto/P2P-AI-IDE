/**
 * viewMessages.ts -- checks the messages the chat view's webview posts to the extension (pure, testable).
 *
 * Purpose: the webview runs untrusted gateway text on screen, so whatever it posts is treated as untrusted input.
 * Security: allow-listed message types; the chat text must be a non-blank string of at most 256 KiB (CWE-20,
 * CWE-400); a change path (X2) must be a string of at most 1024 bytes, and is then only looked up among the pending
 * changes, never opened as a path (CWE-22). Anything else is ignored.
 * Dependencies: ./gateway/checks.ts.
 */
import { MAX_MESSAGE_BYTES } from "./gateway/checks.ts";

/** What the webview may send. Anything else is ignored. */
export type FromView =
  | { type: "ready" }
  | { type: "send"; text: string }
  | { type: "stop" }
  | { type: "new" }
  | { type: "pickGateway" }
  | { type: "pickModel" }
  | { type: "tools"; on: boolean }
  | { type: "review" | "accept" | "reject"; rel: string };

/** Checks a message from the webview (CWE-20). */
export function parseFromView(m: unknown): FromView | undefined {
  if (typeof m !== "object" || m === null) return undefined;
  const o = m as Record<string, unknown>;
  switch (o.type) {
    case "ready":
    case "stop":
    case "new":
    case "pickGateway":
    case "pickModel":
      return { type: o.type };
    case "send":
      if (typeof o.text !== "string") return undefined;
      // SECURITY: the text is capped before it is kept or sent (CWE-400).
      if (o.text.trim() === "" || Buffer.byteLength(o.text) > MAX_MESSAGE_BYTES) return undefined;
      return { type: "send", text: o.text };
    case "tools":
      return typeof o.on === "boolean" ? { type: "tools", on: o.on } : undefined;
    case "review":
    case "accept":
    case "reject":
      // SECURITY: a bounded string; the extension only looks it up among the pending changes (CWE-20, CWE-22).
      if (typeof o.rel !== "string" || o.rel === "" || Buffer.byteLength(o.rel) > 1024) return undefined;
      return { type: o.type, rel: o.rel };
    default:
      return undefined;
  }
}

// ---- the Missions view (X3)

/** What the Missions view's webview may send. Anything else is ignored. */
export type FromMissions =
  | { type: "ready" | "refresh" | "projects" | "unlink" | "push" | "collect" | "pull" | "models" }
  | { type: "link"; id: string; name: string }
  | { type: "create"; name: string }
  | { type: "switches"; autopilot: boolean; autoImprove: boolean }
  | { type: "start"; kind: "autopilot" | "improve"; goal: string; model: string; checks: { id: string; kind: string; framework?: string; argv?: string[] }[] }
  | { type: "control"; id: string; action: "pause" | "resume" | "stop" }
  | { type: "approvals"; runId: string }
  | { type: "decide"; runId: string; approvalId: string; decision: "approve" | "reject" }
  | { type: "proposal"; id: string; action: "accept" | "dismiss" | "snooze" };

const ID = /^[A-Za-z0-9_-]{1,64}$/;
const idv = (x: unknown): x is string => typeof x === "string" && ID.test(x);
const textv = (x: unknown, max: number): x is string => typeof x === "string" && Buffer.byteLength(x) <= max;

/**
 * Checks a message from the Missions webview (CWE-20). Ids must match the id pattern before they are used in a
 * URL path (CWE-74); texts are capped (CWE-400). The approval digest is NOT taken from the view: the extension
 * uses the one it fetched, so the decision is bound to the arguments it showed.
 */
export function parseFromMissions(m: unknown): FromMissions | undefined {
  if (typeof m !== "object" || m === null) return undefined;
  const o = m as Record<string, unknown>;
  switch (o.type) {
    case "ready":
    case "refresh":
    case "projects":
    case "unlink":
    case "push":
    case "collect":
    case "pull":
    case "models":
      return { type: o.type };
    case "link":
      return idv(o.id) && textv(o.name, 512) ? { type: "link", id: o.id, name: o.name } : undefined;
    case "create":
      return textv(o.name, 512) && o.name.trim() !== "" ? { type: "create", name: o.name } : undefined;
    case "switches":
      return typeof o.autopilot === "boolean" && typeof o.autoImprove === "boolean" ? { type: "switches", autopilot: o.autopilot, autoImprove: o.autoImprove } : undefined;
    case "start": {
      if ((o.kind !== "autopilot" && o.kind !== "improve") || !textv(o.goal, 8192) || !textv(o.model, 256) || !Array.isArray(o.checks) || o.checks.length > 8) return undefined;
      const checks: { id: string; kind: string; framework?: string; argv?: string[] }[] = [];
      for (const c of o.checks) {
        if (typeof c !== "object" || c === null) return undefined;
        const k = c as Record<string, unknown>;
        if (!textv(k.id, 64) || !textv(k.kind, 16)) return undefined;
        if (k.framework !== undefined && !textv(k.framework, 32)) return undefined;
        if (k.argv !== undefined && !(Array.isArray(k.argv) && k.argv.length <= 32 && k.argv.every((a) => textv(a, 512)))) return undefined;
        checks.push({ id: k.id, kind: k.kind, framework: k.framework as string | undefined, argv: k.argv as string[] | undefined });
      }
      return { type: "start", kind: o.kind, goal: o.goal, model: o.model, checks };
    }
    case "control":
      return idv(o.id) && (o.action === "pause" || o.action === "resume" || o.action === "stop") ? { type: "control", id: o.id, action: o.action } : undefined;
    case "approvals":
      return idv(o.runId) ? { type: "approvals", runId: o.runId } : undefined;
    case "decide":
      return idv(o.runId) && idv(o.approvalId) && (o.decision === "approve" || o.decision === "reject")
        ? { type: "decide", runId: o.runId, approvalId: o.approvalId, decision: o.decision }
        : undefined;
    case "proposal":
      return idv(o.id) && (o.action === "accept" || o.action === "dismiss" || o.action === "snooze") ? { type: "proposal", id: o.id, action: o.action } : undefined;
    default:
      return undefined;
  }
}
