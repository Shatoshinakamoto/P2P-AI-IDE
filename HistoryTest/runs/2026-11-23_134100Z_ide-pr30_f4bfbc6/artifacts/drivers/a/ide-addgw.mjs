import { randomBytes } from 'node:crypto';
export default async (page) => {
  await page.getByLabel('Gateway name').fill('Ollama dev');
  await page.getByLabel('Gateway address').fill('http://127.0.0.1:11434');
  // Throwaway key for the local Ollama (it ignores auth); never reused.
  await page.getByLabel('Gateway key').fill('throwaway-' + randomBytes(8).toString('hex'));
  await page.getByRole('button', { name: 'Add by address' }).click();
  await page.waitForTimeout(2500);
  return 'submitted';
};
