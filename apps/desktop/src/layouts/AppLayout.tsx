import { NavLink, Outlet } from "react-router-dom";
import { ROUTES, ROUTE_KEYS } from "../routes";
import { useAppInfo } from "../hooks/useAppInfo";

export default function AppLayout() {
  const { info } = useAppInfo();
  return (
    <div className="flex h-full">
      <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-surface">
        <div className="px-5 py-5 text-lg font-bold tracking-tight">
          <span className="text-accent">VPS</span>Pilot
        </div>
        <nav className="flex-1 space-y-1 px-3" aria-label="Main">
          {ROUTE_KEYS.map((key) => (
            <NavLink
              key={key}
              to={ROUTES[key].path}
              end={ROUTES[key].path === "/"}
              className={({ isActive }) =>
                `block rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                  isActive
                    ? "bg-accent/15 text-accent"
                    : "text-muted hover:bg-surface-2 hover:text-fg"
                }`
              }
            >
              {ROUTES[key].label}
            </NavLink>
          ))}
        </nav>
        <div className="border-t border-border px-5 py-3 text-xs text-muted">
          {info ? `v${info.version}` : "…"}
        </div>
      </aside>
      <main className="flex-1 overflow-y-auto p-8">
        <Outlet />
      </main>
    </div>
  );
}
