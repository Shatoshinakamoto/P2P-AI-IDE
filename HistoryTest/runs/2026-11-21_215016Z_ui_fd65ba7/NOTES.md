# Notes for 2026-11-21_215016Z_ui_fd65ba7

## 2026-11-21 21:50 UTC

Result: 98 of 98 checks passed. The web extension reported: net=blocked ipc=blocked tauri=none dom=none open=refused local=open. local=open means it can fetch the app's own bundled files ('self'), which are public and read-only; accepted and documented in docs/features/extensions.md. Not covered here: the real Tauri webviews (WebView2, WebKitGTK, WKWebView); that check is on the PC list.
