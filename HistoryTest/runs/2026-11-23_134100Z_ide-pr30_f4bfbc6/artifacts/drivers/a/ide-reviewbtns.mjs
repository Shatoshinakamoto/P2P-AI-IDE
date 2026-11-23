export default async (page) => {
  const btns = await page.evaluate(() => [...document.querySelectorAll('button')].filter(b=>b.offsetParent!==null).map(b=>`${b.getAttribute('aria-label')??''}|${b.getAttribute('title')??''}|${b.innerText.trim().slice(0,30)}`).filter(t=>/review|diff|discard|reject|accept|apply|view|show/i.test(t)));
  return btns.join('\n');
};
