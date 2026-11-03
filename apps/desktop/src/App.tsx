import { HashRouter, Route, Routes } from "react-router-dom";
import AppLayout from "./layouts/AppLayout";
import { ROUTES } from "./routes";
import DashboardPage from "./pages/DashboardPage";
import ServersPage from "./pages/ServersPage";
import DockerPage from "./pages/DockerPage";
import TerminalPage from "./pages/TerminalPage";
import LogsPage from "./pages/LogsPage";
import AlertsPage from "./pages/AlertsPage";
import SettingsPage from "./pages/SettingsPage";

export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<DashboardPage />} />
          <Route path={ROUTES.servers.path} element={<ServersPage />} />
          <Route path={ROUTES.docker.path} element={<DockerPage />} />
          <Route path={ROUTES.terminal.path} element={<TerminalPage />} />
          <Route path={ROUTES.logs.path} element={<LogsPage />} />
          <Route path={ROUTES.alerts.path} element={<AlertsPage />} />
          <Route path={ROUTES.settings.path} element={<SettingsPage />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
