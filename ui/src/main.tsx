import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { desktopApi, type ClientSettings } from "./api";
import { applyTheme } from "./theme";

const DEFAULT_SETTINGS: ClientSettings = {
  autoStart: true,
  startupConsentGranted: false,
  port: 16729,
  launchAtLogin: false,
  closeToTray: true,
  language: "system",
  theme: "system",
};

void desktopApi.getClientSettings().catch(() => DEFAULT_SETTINGS).then((settings) => {
  applyTheme(settings.theme ?? "system");
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App initialSettings={settings} />
    </React.StrictMode>,
  );
});
