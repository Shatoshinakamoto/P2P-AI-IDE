// Keeps a CDP connection open and appends console messages / page errors to a file.
// Usage: node log.mjs <port> <logfile>
import { chromium } from 'playwright-core';
import { appendFileSync } from 'node:fs';

const [port, file] = process.argv.slice(2);
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const ts = () => new Date().toISOString();
const hook = (p) => {
  p.on('console', (m) => appendFileSync(file, `${ts()} [${m.type()}] ${m.text()}\n`));
  p.on('pageerror', (e) => appendFileSync(file, `${ts()} [pageerror] ${e.message}\n`));
  p.on('requestfailed', (r) => appendFileSync(file, `${ts()} [requestfailed] ${r.url().split('?')[0]} ${r.failure()?.errorText}\n`));
};
for (const c of browser.contexts()) { c.pages().forEach(hook); c.on('page', hook); }
appendFileSync(file, `${ts()} [logger] attached\n`);
setInterval(() => {}, 1 << 30);
