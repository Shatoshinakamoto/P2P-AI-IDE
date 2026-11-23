export default async (page) => {
  await page.getByText('hello.py', { exact: true }).first().click();
  await page.waitForTimeout(2500);
  const lines = await page.locator('.monaco-editor .view-lines').first().innerText().catch(() => 'NO MONACO');
  return 'editor lines: ' + JSON.stringify(lines.slice(0, 200));
};
