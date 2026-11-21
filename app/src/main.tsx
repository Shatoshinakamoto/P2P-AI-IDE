/**
 * main.tsx -- starts the editor services (monaco.ts), then mounts the React app into #root.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App.tsx";
import { setColorTheme, startEditor } from "./monaco.ts";
import { syncExtensions } from "./extensions.ts";
import { savedTheme } from "./lib/theme.ts";

const root = document.getElementById("root");
// The editor services start before the first render, so every editor finds them ready.
if (root) {
  await startEditor();
  // Installed extensions and the saved theme load in the background; the window does not wait for them.
  void syncExtensions()
    .catch(() => undefined)
    .then(() => {
      const theme = savedTheme();
      if (theme) void setColorTheme(theme);
    });
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
