export const ROUTES = {
  dashboard: { path: "/", label: "Dashboard" },
  servers: { path: "/servers", label: "Servers" },
  docker: { path: "/docker", label: "Docker" },
  terminal: { path: "/terminal", label: "Terminal" },
  logs: { path: "/logs", label: "Logs" },
  alerts: { path: "/alerts", label: "Alerts" },
  settings: { path: "/settings", label: "Settings" },
} as const;

export type RouteKey = keyof typeof ROUTES;
export const ROUTE_KEYS = Object.keys(ROUTES) as RouteKey[];
