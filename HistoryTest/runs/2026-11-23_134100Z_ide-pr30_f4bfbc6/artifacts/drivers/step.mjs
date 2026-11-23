// Click-through driver: attach to a WebView2 over CDP, run one step's actions,
// take a screenshot, print the page's visible text. Local test tool only.
// Usage: node step.mjs <port> <png-path> <actions.mjs | -> [textLimit]
import { chromium } from 'playwright-core';
import { pathToFileURL } from 'node:url';

const [port, png, actions, limit = '3000'] = process.argv.slice(2);
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const pages = browser.contexts().flatMap((c) => c.pages());
const page = pages.find((p) => !p.url().startsWith('devtools')) ?? pages[0];
let out = '';
try {
  if (actions && actions !== '-') {
    const mod = await import(pathToFileURL(actions).href + `?t=${Date.now()}`);
    out = (await mod.default(page)) ?? '';
  }
} catch (e) {
  out = 'ACTION ERROR: ' + (e?.message ?? String(e)).split('\n')[0];
}
await page.waitForTimeout(500);
await page.screenshot({ path: png });
const text = await page.evaluate(() => document.body?.innerText ?? '');
console.log('URL', page.url());
if (out) console.log('RESULT', out);
console.log('TEXT', text.slice(0, Number(limit)));
process.exit(0);
