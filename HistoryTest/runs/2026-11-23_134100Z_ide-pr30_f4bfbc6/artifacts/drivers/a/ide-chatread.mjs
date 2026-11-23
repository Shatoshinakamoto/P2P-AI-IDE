// Reads the Agent chat transcript region (text + any JSON-looking lines) and screenshots only the chat panel.
export default async (page) => {
  const panel = page.locator('textarea[aria-label="Message"]').locator('xpath=ancestor::*[.//button[@aria-label="Tools"]][1]');
  await panel.screenshot({ path: process.env.CHATPNG });
  const txt = await panel.innerText();
  const json = txt.split('\n').filter((l) => /^\s*\{.*"(name|arguments|tool)"/.test(l));
  return JSON.stringify({ json_lines: json, transcript: txt.slice(0, 4000) }, null, 1);
};
