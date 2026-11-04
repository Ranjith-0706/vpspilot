import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ThemeProvider } from "./hooks/useTheme";
import { MonitoringProvider } from "./hooks/useMonitoring";
import "./styles/index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <MonitoringProvider>
        <App />
      </MonitoringProvider>
    </ThemeProvider>
  </React.StrictMode>,
);
