# abi-probe (test fixture, phase I6d)

A tiny VS Code extension whose native module (`native/probe.cc`) uses Node's classic C++ interface, built for
Electron 43.8.3 (module version 148). It loads only in Electron's Node mode, never in our server's own Node 24
(module version 137), so it shows whether the Electron compatibility host is in use. Not part of the app.

Build (needs a C++ compiler and Python; downloads Electron's headers from electronjs.org):

```sh
cd app/tests/fixtures/abi-probe/native
npx -y node-gyp@11.5.0 rebuild --target=43.8.3 --dist-url=https://electronjs.org/headers --runtime=electron
```

Then run `app/tests/electron-host-e2e.sh` (see `docs/commands/electron-host-e2e.md`). The build output
(`native/build/`) is ignored by git.
