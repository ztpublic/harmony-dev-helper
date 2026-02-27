import { resolveBootstrap } from "@harmony/webview-bridge";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { readAppSettings } from "./features/settings/appSettings";
import "./styles.css";

const bootstrap = resolveBootstrap(window);
const appSettings = readAppSettings(bootstrap.host);
document.documentElement.dataset.theme = appSettings.theme;
document.documentElement.style.colorScheme = appSettings.theme;

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>
);
