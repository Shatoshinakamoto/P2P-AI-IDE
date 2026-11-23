/**
 * nexus.ts -- loopback discovery and /health verification for a local NEXUS Engine in the VS Code extension.
 *
 * Purpose: detects and connects to NEXUS Proxy (default `11435`, Agent tools + Context Packer + Data Airlock)
 * and NEXUS Runtime (default `11436`, chat-only native `.nexus` inference) running on `127.0.0.1` without keys.
 *
 * Security:
 *  - Loopback only (`127.0.0.1`, canonical unprivileged port `1024..=65535`); refuses remote hosts, paths,
 *    queries, and userinfo (NIST SC-7, CWE-918, CWE-20).
 *  - Reads `~/.nexus-studio/studio-settings.json` only via `lstatSync` (no symlinks, CWE-59) and capped at 64 KiB
 *    (CWE-400).
 *  - Probes `/health` using explicit Node `http.request` with no redirects (`3xx` refused, CWE-601), short 2 s
 *    timeout, 64 KiB cap, and fixed error messages containing no raw paths or response content (CWE-209).
 * Dependencies: Node `fs`, `http`, `os`, `path`, `./gateway/checks.ts`.
 */
import * as fs from "node:fs";
import * as http from "node:http";
import * as os from "node:os";
import * as path from "node:path";
import { isNexusLoopbackUrl } from "./gateway/checks.ts";

export const DEFAULT_NEXUS_PROXY_PORT = 11435;
export const DEFAULT_NEXUS_RUNTIME_PORT = 11436;
const MAX_SETTINGS_BYTES = 64 << 10;
const MAX_HEALTH_BYTES = 64 << 10;

export type NexusServiceKind = "nexus-proxy" | "nexus-runtime";

export interface NexusProbeResult {
  service: NexusServiceKind;
  url: string;
  running: boolean;
  name: string;
  modelsLoaded?: number;
  modelsAvailable?: number;
  simd?: string;
}

/** Validates an unprivileged local port `1024..=65535` and returns `http://127.0.0.1:<port>`, or null. */
export function nexusUrlForPort(port: number): string | null {
  if (!Number.isInteger(port) || port < 1024 || port > 65535) return null;
  return `http://127.0.0.1:${port}`;
}

/** Normalizes a loopback URL from `~/.nexus-studio/studio-settings.json` to `http://127.0.0.1:<port>`, or null. */
export function parseNexusSettingsUrl(raw: string): string | null {
  const trimmed = raw.trim().replace(/\/$/, "");
  const m = /^http:\/\/(127\.0\.0\.1|localhost|\[::1\]):([1-9]\d{3,4})$/i.exec(trimmed);
  if (!m) return null;
  return nexusUrlForPort(Number(m[2]));
}

/** Canonical saved gateway display name matching `core/src/nexus.rs`. */
export function nexusDisplayName(service: NexusServiceKind, url: string): string {
  const base = service === "nexus-proxy" ? "NEXUS Proxy" : "NEXUS Runtime";
  const defPort = service === "nexus-proxy" ? DEFAULT_NEXUS_PROXY_PORT : DEFAULT_NEXUS_RUNTIME_PORT;
  if (url === `http://127.0.0.1:${defPort}`) return `${base} (this computer)`;
  const port = url.slice(url.lastIndexOf(":") + 1);
  return `${base} (this computer :${port})`;
}

/** Discovers loopback endpoints from `~/.nexus-studio/studio-settings.json` or falls back to `11435` / `11436`. */
export function discoverNexusEndpoints(homeDir = os.homedir()): { proxyUrl: string; runtimeUrl: string } {
  const out = {
    proxyUrl: `http://127.0.0.1:${DEFAULT_NEXUS_PROXY_PORT}`,
    runtimeUrl: `http://127.0.0.1:${DEFAULT_NEXUS_RUNTIME_PORT}`,
  };
  try {
    const file = path.join(homeDir, ".nexus-studio", "studio-settings.json");
    const st = fs.lstatSync(file);
    // SECURITY: refuse symlinks and oversized config files (CWE-59, CWE-400).
    if (!st.isFile() || st.size > MAX_SETTINGS_BYTES) return out;
    const raw = fs.readFileSync(file, "utf8");
    const doc = JSON.parse(raw) as { proxyUrl?: unknown; runtimeUrl?: unknown };
    if (typeof doc.proxyUrl === "string") {
      const p = parseNexusSettingsUrl(doc.proxyUrl);
      if (p) out.proxyUrl = p;
    }
    if (typeof doc.runtimeUrl === "string") {
      const r = parseNexusSettingsUrl(doc.runtimeUrl);
      if (r) out.runtimeUrl = r;
    }
  } catch {
    // Missing or unreadable settings fall back cleanly to defaults.
  }
  return out;
}

function cleanAsciiTag(v: unknown, max = 32): string | undefined {
  if (typeof v !== "string") return undefined;
  const cleaned = v
    .trim()
    .split("")
    .filter((c) => c.charCodeAt(0) >= 0x20 && c.charCodeAt(0) <= 0x7e)
    .slice(0, max)
    .join("");
  return cleaned || undefined;
}

/** Probes `<url>/health` on `127.0.0.1` and verifies `status === "ok"` and `service === expected`. */
export function probeNexusEndpoint(url: string, expected: NexusServiceKind): Promise<NexusProbeResult> {
  const name = nexusDisplayName(expected, url);
  if (!isNexusLoopbackUrl(url)) {
    return Promise.resolve({ service: expected, url, running: false, name });
  }
  const u = new URL(`${url}/health`);
  return new Promise((resolve) => {
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port: Number(u.port),
        path: "/health",
        method: "GET",
        timeout: 2000,
        headers: { Accept: "application/json", "User-Agent": "p2p-ai-vscode/0.1" },
      },
      (res) => {
        if (!res.statusCode || res.statusCode < 200 || res.statusCode >= 300) {
          res.resume();
          return resolve({ service: expected, url, running: false, name });
        }
        const chunks: Buffer[] = [];
        let total = 0;
        res.on("data", (d: Buffer) => {
          total += d.length;
          if (total > MAX_HEALTH_BYTES) {
            req.destroy();
            return resolve({ service: expected, url, running: false, name });
          }
          chunks.push(d);
        });
        res.on("end", () => {
          try {
            const doc = JSON.parse(Buffer.concat(chunks).toString("utf8")) as Record<string, unknown>;
            if (doc.status !== "ok" || doc.service !== expected) {
              return resolve({ service: expected, url, running: false, name });
            }
            const out: NexusProbeResult = { service: expected, url, running: true, name };
            if (expected === "nexus-runtime") {
              if (typeof doc.models_loaded === "number") out.modelsLoaded = doc.models_loaded;
              if (typeof doc.models_available === "number") out.modelsAvailable = doc.models_available;
              const simd = cleanAsciiTag(doc.simd);
              if (simd) out.simd = simd;
            }
            resolve(out);
          } catch {
            resolve({ service: expected, url, running: false, name });
          }
        });
        res.on("error", () => resolve({ service: expected, url, running: false, name }));
      },
    );
    req.on("timeout", () => {
      req.destroy();
      resolve({ service: expected, url, running: false, name });
    });
    req.on("error", () => resolve({ service: expected, url, running: false, name }));
    req.end();
  });
}
