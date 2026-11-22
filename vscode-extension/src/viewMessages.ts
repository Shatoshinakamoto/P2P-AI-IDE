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
