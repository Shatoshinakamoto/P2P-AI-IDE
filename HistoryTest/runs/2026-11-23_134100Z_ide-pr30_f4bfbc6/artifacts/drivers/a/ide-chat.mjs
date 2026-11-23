// Picks MODEL, sends PROMPT, waits until the Send button is back (turn finished) or 240 s.
export default async (page) => {
  const model = process.env.MODEL;
  await page.getByLabel('Model').selectOption(model);
  const box = page.locator('textarea[aria-label="Message"]');
  await box.fill(process.env.CHAT_PROMPT);
  const t0 = Date.now();
  await box.press('Control+Enter');
  await page.waitForTimeout(3000);
  // While streaming the Send button turns into Stop.
  for (let i = 0; i < 240; i++) {
    const stop = await page.getByRole('button', { name: 'Stop', exact: true }).count();
    if (!stop) break;
    await page.waitForTimeout(1000);
  }
  await page.waitForTimeout(1500);
  return `model=${model} seconds=${((Date.now() - t0) / 1000).toFixed(1)}`;
};
