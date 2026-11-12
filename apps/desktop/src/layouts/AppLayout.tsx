import type { ReactNode } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { ROUTES, ROUTE_KEYS, type RouteKey } from "../routes";
import { useAppInfo } from "../hooks/useAppInfo";

/** 24x24 stroke icons (Lucide-style paths), drawn inline so there is no icon dependency. */
const ICONS: Record<RouteKey, ReactNode> = {
  dashboard: (
    <>
      <rect x="3" y="3" width="7" height="9" rx="1" />
      <rect x="14" y="3" width="7" height="5" rx="1" />
      <rect x="14" y="12" width="7" height="9" rx="1" />
      <rect x="3" y="16" width="7" height="5" rx="1" />
    </>
  ),
  servers: (
    <>
      <rect x="3" y="3" width="18" height="7" rx="1" />
      <rect x="3" y="14" width="18" height="7" rx="1" />
      <path d="M7 6.5h.01M7 17.5h.01" />
    </>
  ),
  docker: (
    <>
      <path d="M21 8 12 3 3 8v8l9 5 9-5z" />
      <path d="m3 8 9 5 9-5M12 13v8" />
    </>
  ),
  terminal: (
    <>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="m7 9 3 3-3 3M13 15h4" />
    </>
  ),
  logs: (
    <>
      <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
      <path d="M14 3v6h6M8 13h8M8 17h6" />
    </>
  ),
  alerts: (
    <>
      <path d="M6 8a6 6 0 1 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
      <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h0a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h0a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v0a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </>
  ),
};

function NavIcon({ name }: { name: RouteKey }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className="shrink-0"
    >
      {ICONS[name]}
    </svg>
  );
}

export default function AppLayout() {
  const { info } = useAppInfo();
  const location = useLocation();
  const current = ROUTE_KEYS.find((k) =>
    ROUTES[k].path === "/"
      ? location.pathname === "/"
      : location.pathname.startsWith(ROUTES[k].path),
  );

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-0 flex-1">
        {/* Icon rail on narrow windows, full sidebar from 1000px up. */}
        <aside className="flex w-12 shrink-0 flex-col border-r border-border bg-surface min-[1000px]:w-52">
          <div className="flex h-11 items-center justify-center border-b border-border text-sm font-bold tracking-tight min-[1000px]:justify-start min-[1000px]:px-4">
            <span className="text-accent">VPS</span>
            <span className="hidden min-[1000px]:inline">Pilot</span>
          </div>
          <nav className="flex-1 space-y-0.5 overflow-y-auto p-1.5" aria-label="Main">
            {ROUTE_KEYS.map((key) => (
              <NavLink
                key={key}
                to={ROUTES[key].path}
                end={ROUTES[key].path === "/"}
                title={ROUTES[key].label}
                aria-label={ROUTES[key].label}
                className={({ isActive }) =>
                  `flex items-center justify-center gap-2.5 rounded-md border-l-2 px-2 py-2 text-sm font-medium transition-colors min-[1000px]:justify-start min-[1000px]:px-2.5 ${
                    isActive
                      ? "border-accent bg-accent/15 text-accent"
                      : "border-transparent text-muted hover:bg-surface-2 hover:text-fg"
                  }`
                }
              >
                <NavIcon name={key} />
                <span className="hidden min-[1000px]:inline">{ROUTES[key].label}</span>
              </NavLink>
            ))}
          </nav>
        </aside>
        <main className="min-w-0 flex-1 overflow-y-auto p-3 sm:p-4 min-[1000px]:p-5">
          <Outlet />
        </main>
      </div>
      <footer className="flex h-6 shrink-0 items-center justify-between border-t border-border bg-surface px-3 text-xs text-muted">
        <span>{current ? ROUTES[current].label : "VPSPilot"}</span>
        <span>{info ? `VPSPilot v${info.version}` : "VPSPilot"}</span>
      </footer>
    </div>
  );
}
