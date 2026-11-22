# P2P-AI for VS Code and VSCodium

Chat with the models on a P2P-AI gateway from the side bar.

1. Run **P2P-AI: Add Gateway** and enter the gateway's name, address (`https://host[:port]`) and key. If the gateway
   uses its own certificate authority, pick its CA file.
2. Open the **P2P-AI** view, pick a model, and type.

The key is kept in VS Code's secret storage (the OS keychain where available) and is sent only to that gateway.
See `docs/features/vscode-extension.md` in the P2P-AI IDE repository.
