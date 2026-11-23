export default async (page) => {
  const items = await page.evaluate(() =>
    [...document.querySelectorAll('button, [role=button], [role=tab], input, select, textarea')]
      .filter((e) => e.offsetParent !== null)
      .map((e) => `${e.tagName.toLowerCase()}|${e.getAttribute('aria-label') ?? ''}|${e.getAttribute('title') ?? ''}|${e.getAttribute('placeholder') ?? ''}|${(e.innerText ?? '').trim().slice(0, 40)}`)
  );
  return '\n' + items.join('\n');
};
