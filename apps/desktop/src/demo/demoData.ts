// ALL DATA IN THIS FOLDER IS DEMO DATA FOR UI DEVELOPMENT ONLY.
// Hostnames use the reserved .example TLD; nothing here is real.
import type { DemoServer } from "../types";

export const mockServers: DemoServer[] = [
  {
    id: "s1",
    name: "web-01",
    host: "web-01.example",
    region: "Frankfurt",
    os: "Ubuntu 24.04",
    status: "online",
    cpuPercent: 34,
    ramPercent: 58,
    diskPercent: 41,
    uptime: "42d 6h",
  },
  {
    id: "s2",
    name: "db-01",
    host: "db-01.example",
    region: "Frankfurt",
    os: "Debian 12",
    status: "warning",
    cpuPercent: 78,
    ramPercent: 86,
    diskPercent: 72,
    uptime: "118d 2h",
  },
  {
    id: "s3",
    name: "cache-01",
    host: "cache-01.example",
    region: "Amsterdam",
    os: "Ubuntu 22.04",
    status: "online",
    cpuPercent: 12,
    ramPercent: 31,
    diskPercent: 18,
    uptime: "9d 14h",
  },
  {
    id: "s4",
    name: "staging-01",
    host: "staging-01.example",
    region: "Singapore",
    os: "Rocky Linux 9",
    status: "offline",
    cpuPercent: 0,
    ramPercent: 0,
    diskPercent: 0,
    uptime: "-",
  },
];
