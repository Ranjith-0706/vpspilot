export type Status = "online" | "offline" | "warning";
export type ThemePreference = "light" | "dark" | "system";
export type LogLevel = "info" | "warn" | "error" | "debug";
export type AlertSeverity = "critical" | "warning" | "info";

export interface DemoServer {
  id: string;
  name: string;
  host: string;
  region: string;
  os: string;
  status: Status;
  cpuPercent: number;
  ramPercent: number;
  diskPercent: number;
  uptime: string;
}

export interface Container {
  id: string;
  name: string;
  image: string;
  state: "running" | "exited" | "restarting";
  ports: string;
  uptime: string;
}

export interface LogEntry {
  id: string;
  timestamp: string;
  level: LogLevel;
  source: string;
  message: string;
}

export interface AlertItem {
  id: string;
  severity: AlertSeverity;
  title: string;
  server: string;
  time: string;
  resolved: boolean;
}

export interface AppInfo {
  name: string;
  version: string;
}
