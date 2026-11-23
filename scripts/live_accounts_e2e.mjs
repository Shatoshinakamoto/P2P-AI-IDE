#!/usr/bin/env node
/**
 * live_accounts_e2e.mjs -- end-to-end check of "Sign in with a P2P-AI account" (ADR-030 phase A3) in the IDE.
 *
 * Purpose: the core's unit tests use a fake home. This script runs the real parts on 127.0.0.1:
 *
 *   IDE core (cargo example live_accounts)  --loopback PKCE-->  p2p-accounts (real Go account service)
 *        |                                                           ^
 *        |  Bearer <5-minute account token>                          |  headless Chromium signs in on its page
 *        v                                                           |
 *   Go gateway (GATEWAY_ACCOUNTS_ACCEPT=on, pinned home)  -> tiny fake model node (this script)
 *
 * The person is played by Playwright's Chromium: it opens the sign-in link the IDE core printed, types the email and
 * the one-time password, and follows the redirect back to the core's loopback listener. The core then changes the
 * password, gets a gateway token, lists models, streams a chat, and signs out. Each check prints PASS or FAIL.
 *
 * Usage: node scripts/live_accounts_e2e.mjs --bin-dir DIR [--chromium PATH]
 *   DIR holds `p2p-accounts` and `gateway` built from the P2P-AI checkout's P2P-AI-Gateways/Go.
 * Exit: 0 all passed, 1 a check failed, 2 setup error. Docs: docs/commands/live-accounts-e2e.md.
 *
 * Security notes:
 *   - SECURITY-WARNING: plain http on 127.0.0.1 only (ACCOUNTS_ALLOW_INSECURE_HTTP=on, INSECURE — DEV ONLY).
 *   - SECURITY: every secret (admin keys, enrollment token, accounts key, passwords) is random per run from the
 *     CSPRNG (node:crypto), goes to children through owner-only files or their environment / stdin, never argv, and
 *     is never printed (CWE-214, CWE-532, CWE-798).
 *   - SECURITY: child processes get argument arrays, never a shell string (CWE-78).
 * Dependencies: Node 20+, playwright (the copy installed on the machine), cargo, the two Go binaries.
 */
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const GATEWAY_ID = 'gw-ide-accounts-e2e';
const EMAIL = 'ide-e2e@example.org';

const args = process.argv.slice(2);
const arg = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : undefined; };
const binDir = arg('--bin-dir');
const chromiumPath = arg('--chromium');
if (!binDir) { console.error('usage: --bin-dir DIR [--chromium PATH]'); process.exit(2); }

let fails = 0, passes = 0;
function check(name, ok, detail = '') {
  if (ok) passes++; else fails++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${!ok && detail ? `  (${String(detail).slice(0, 300)})` : ''}`);
}
const rnd = (n = 32) => crypto.randomBytes(n).toString('base64url');
const freePort = () => new Promise((res) => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); });
function secretFile(p, v) { fs.writeFileSync(p, v + '\n', { mode: 0o600, flag: 'wx' }); }

async function waitUp(url, tries = 100) {
  for (let i = 0; i < tries; i++) {
    try { const r = await fetch(url); if (r.status < 500) return true; } catch { /* not yet */ }
    await new Promise((r) => setTimeout(r, 100));
  }
  return false;
}
async function api(base, method, p, key, body) {
  const r = await fetch(base + p, { method, headers: { ...(key ? { Authorization: `Bearer ${key}` } : {}), ...(body ? { 'Content-Type': 'application/json' } : {}) }, body: body ? JSON.stringify(body) : undefined });
  let j = {}; try { j = await r.json(); } catch { /* empty */ }
  return [r.status, j];
}

// A tiny Ollama-shaped node: /api/tags and /api/chat with a fixed answer.
const model = http.createServer((req, res) => {
  const chunks = []; let n = 0;
  req.on('data', (c) => { n += c.length; if (n > 1 << 20) req.destroy(); else chunks.push(c); });
  req.on('end', () => {
    if (req.method === 'GET') { res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify({ models: [{ name: 'm', model: 'm' }] })); return; }
    let stream = false; try { stream = JSON.parse(Buffer.concat(chunks).toString()).stream === true; } catch { /* not JSON */ }
    const text = { model: 'm', message: { role: 'assistant', content: 'hello from the fake model' }, done: !stream };
    const last = { model: 'm', message: { role: 'assistant', content: '' }, done: true, done_reason: 'stop', prompt_eval_count: 7, eval_count: 5 };
    res.writeHead(200, { 'Content-Type': stream ? 'application/x-ndjson' : 'application/json' });
    // Ollama streams one JSON object per line and ends with done: true.
    res.end(stream ? `${JSON.stringify(text)}\n${JSON.stringify(last)}\n` : JSON.stringify({ ...text, prompt_eval_count: 7, eval_count: 5 }));
  });
});

const procs = [];
function start(bin, env, extra = {}) {
  const p = spawn(bin, extra.args || [], { env: { PATH: process.env.PATH, HOME: process.env.HOME, ...env }, stdio: extra.stdio || ['ignore', 'ignore', 'pipe'], cwd: extra.cwd });
  p.stderrText = ''; p.stderr?.on('data', (d) => { p.stderrText = (p.stderrText + d).slice(-4000); });
  procs.push(p); return p;
}

async function main() {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'ide-acct-e2e-'));
  fs.chmodSync(work, 0o700);
  let browser;
  try {
    await new Promise((r) => model.listen(0, '127.0.0.1', r));
    // ---- account service ----
    const aPort = await freePort();
    const issuer = `http://127.0.0.1:${aPort}`;
    const aAdmin = rnd();
    secretFile(path.join(work, 'acct-admin.key'), aAdmin);
    start(path.join(binDir, 'p2p-accounts'), { ACCOUNTS_LISTEN: `127.0.0.1:${aPort}`, ACCOUNTS_ISSUER: issuer, ACCOUNTS_DATA_DIR: path.join(work, 'acct-data'), ACCOUNTS_ADMIN_KEY_FILE: path.join(work, 'acct-admin.key'), ACCOUNTS_ALLOW_INSECURE_HTTP: 'on' });
    check('account service up', await waitUp(`${issuer}/healthz`));
    const [, keys] = await api(issuer, 'GET', '/v1/keys');
    const homeKey = keys?.keys?.[0]?.x || '';

    // ---- gateway with accounts on, home pinned ----
    const gPort = await freePort();
    const gw = `http://127.0.0.1:${gPort}`;
    const gAdmin = rnd(24), enroll = rnd(24);
    secretFile(path.join(work, 'gw-accounts.key'), rnd());
    start(path.join(binDir, 'gateway'), { GATEWAY_LISTEN: `127.0.0.1:${gPort}`, GATEWAY_ID, GATEWAY_ADMIN_KEY: gAdmin, GATEWAY_ENROLL_TOKEN: enroll, GATEWAY_SIGNING_KEY: crypto.randomBytes(32).toString('hex'), GATEWAY_ATTACHMENT_DIR: path.join(work, 'att'), GATEWAY_WORKSPACE_DIR: path.join(work, 'ws'), GATEWAY_ACCOUNTS_ACCEPT: 'on', GATEWAY_ACCOUNTS_HOMES: `${issuer}|${homeKey}`, GATEWAY_ACCOUNTS_KEY_FILE: path.join(work, 'gw-accounts.key') });
    check('gateway up with accounts on', await waitUp(`${gw}/healthz`));
    const [rs, ro] = await api(gw, 'POST', '/v1/nodes/register', enroll, { name: 'fake', kind: 'gpu', vram_gb: 8, endpoint: `http://127.0.0.1:${model.address().port}`, models: ['m'] });
    check('fake model node registered', rs === 201, JSON.stringify(ro));

    // ---- operator makes the account (one-time password) ----
    const [cs, co] = await api(issuer, 'POST', '/v1/admin/accounts', aAdmin, { email: EMAIL });
    const otp = co.one_time_password || '';
    check('account created by the operator', cs === 201 && otp.length > 0, String(cs));

    // ---- the IDE core, playing the app ----
    const ex = start('cargo', {}, { args: ['run', '-q', '--example', 'live_accounts', '--', '--home', issuer, '--audience', GATEWAY_ID, '--gateway', gw, '--data', path.join(work, 'ide-data')], cwd: path.join(ROOT, 'core'), stdio: ['pipe', 'pipe', 'pipe'] });
    const { chromium } = require('playwright');
    browser = await chromium.launch(chromiumPath ? { executablePath: chromiumPath } : {});
    const rl = readline.createInterface({ input: ex.stdout });
    const done = new Promise((res) => ex.on('exit', (c) => res(c)));
    for await (const line of rl) {
      let m; try { m = JSON.parse(line); } catch { continue; }
      if (m.open) {
        // SECURITY: the link must be the home's own sign-in page with our client and a loopback return.
        const u = new URL(m.open);
        check('IDE opens the home sign-in page with PKCE S256 and a 127.0.0.1 return', u.origin === issuer && u.pathname === '/authorize' && u.searchParams.get('client_id') === 'p2p-ai-ide' && u.searchParams.get('code_challenge_method') === 'S256' && /^http:\/\/127\.0\.0\.1:\d+\/cb$/.test(u.searchParams.get('redirect_uri') || ''), m.open.replace(/state=[^&]+/, 'state=…'));
        const page = await browser.newPage();
        await page.goto(m.open);
        await page.fill('input[name=email]', EMAIL);
        await page.fill('input[name=password]', otp);
        await page.fill('input[name=device]', 'IDE e2e');
        await Promise.all([page.waitForURL(/127\.0\.0\.1:\d+\/cb/), page.click('button[type=submit]')]);
        const h1 = await page.textContent('h1');
        check('browser lands on the IDE\'s "Signed in" page', h1 === 'Signed in', h1);
        await page.close();
      } else if (m.need === 'passwords') {
        ex.stdin.write(`${otp}\n${rnd(18)} long passphrase\n`);
      } else if (m.step) {
        check(m.step, m.ok, m.detail);
      }
    }
    const code = await done;
    check('IDE core example finished cleanly', code === 0, `exit ${code}; ${ex.stderrText.slice(-500)}`);
  } catch (e) {
    console.error('setup error:', e?.message || e);
    fails++;
  } finally {
    if (browser) await browser.close();
    for (const p of procs) { try { p.kill('SIGTERM'); } catch { /* gone */ } }
    model.close();
    fs.rmSync(work, { recursive: true, force: true });
  }
  console.log(`SUMMARY: ${passes} passed, ${fails} failed`);
  process.exit(fails ? 1 : 0);
}
main();
