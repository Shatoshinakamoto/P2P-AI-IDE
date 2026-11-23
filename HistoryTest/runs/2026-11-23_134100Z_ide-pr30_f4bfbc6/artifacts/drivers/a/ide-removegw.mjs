export default async (page) => {
  await page.getByText('Remove', { exact: true }).first().click();
  await page.waitForTimeout(800);
  const after = await page.evaluate(() => [...document.querySelectorAll('button')].map((b) => `${b.innerText.trim()}[${b.getAttribute('aria-label') ?? ''}]`).filter((t) => /remove|confirm|sure/i.test(t)).join(' | '));
  await page.screenshot({ path: process.env.MID });
  const confirm = page.getByText(/^(Confirm|Really remove\??|Click again to remove|Remove\?|Confirm remove)$/i).first();
  if (await confirm.count()) await confirm.click(); else await page.getByText('Remove', { exact: true }).first().click();
  await page.waitForTimeout(1500);
  return 'after first click: ' + after;
};
