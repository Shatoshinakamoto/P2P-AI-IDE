export default async (page) => {
  const txt = await page.evaluate(() => [...document.querySelectorAll('.monaco-diff-editor .view-lines')].map(e=>e.innerText).join('\n=====\n'));
  return txt.slice(0,1500) || 'NO DIFF EDITOR';
};
