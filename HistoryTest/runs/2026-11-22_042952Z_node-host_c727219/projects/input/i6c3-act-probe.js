// i6c3-act-probe.js -- dev check only: asks the remote extension host directly to activate by event, to see how far it got.
(async () => {
  const d = window.__dbg; const S = d.StandaloneServices;
  const ex = S.get(d.IExtensionService);
  const m = [...ex._extensionHostManagers].find((x) => x.kind === 3);
  const p = await m._proxy;
  const t = (pr) => Promise.race([pr.then((v) => "done " + JSON.stringify(v), (e) => "error " + String(e)), new Promise((r) => setTimeout(() => r("pending after 5s"), 5000))]);
  return {
    immediate: await t(p._actual.$activateByEvent("onStartupFinished", 1)),
    normal: await t(p._actual.$activateByEvent("onStartupFinished", 0)),
  };
})()
