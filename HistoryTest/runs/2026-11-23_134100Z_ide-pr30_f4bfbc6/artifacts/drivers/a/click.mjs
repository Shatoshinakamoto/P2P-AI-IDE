// Clicks the control named in env CLICK (aria-label, or visible text with "text:" prefix),
// waits WAIT ms, then lists visible controls.
import list from './list-buttons.mjs';
export default async (page) => {
  const target = process.env.CLICK;
  const wait = Number(process.env.WAIT ?? 1500);
  if (target) {
    const loc = target.startsWith('text:')
      ? page.getByText(target.slice(5), { exact: true }).first()
      : page.getByRole('button', { name: target, exact: true }).first();
    await loc.click({ timeout: 5000 });
  }
  await page.waitForTimeout(wait);
  return process.env.LIST === '0' ? 'clicked' : await list(page);
};
