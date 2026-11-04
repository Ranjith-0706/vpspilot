export type Status = "online" | "offline" | "warning";
export type ThemePreference = "light" | "dark" | "system";

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

export interface AppInfo {
  name: string;
  version: string;
}
