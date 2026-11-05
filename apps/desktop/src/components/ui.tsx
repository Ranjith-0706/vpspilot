import type { ReactNode } from "react";

export function Card({
  title,
  children,
  className = "",
}: {
  title?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-md border border-border bg-surface p-4 ${className}`}>
      {title && (
        <h2 className="mb-3 text-xs font-semibold uppercase tracking-wide text-muted">{title}</h2>
      )}
      {children}
    </section>
  );
}

export function MockBadge() {
  return (
    <span className="rounded border border-warn/40 bg-warn/10 px-2 py-0.5 text-xs font-medium text-warn">
      Mock data
    </span>
  );
}

export function PageHeader({
  title,
  subtitle,
  actions,
  mock = false,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  mock?: boolean;
}) {
  return (
    <header className="mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-border pb-3">
      <div>
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-semibold">{title}</h1>
          {mock && <MockBadge />}
        </div>
        {subtitle && <p className="mt-0.5 max-w-3xl text-xs text-muted">{subtitle}</p>}
      </div>
      {actions}
    </header>
  );
}

import type { Tone } from "./tones";
const toneClass: Record<Tone, string> = {
  ok: "bg-ok/15 text-ok",
  warn: "bg-warn/15 text-warn",
  bad: "bg-bad/15 text-bad",
  neutral: "bg-surface-2 text-muted",
};

export function Badge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span
      className={`inline-block whitespace-nowrap rounded px-2 py-0.5 text-xs font-medium ${toneClass[tone]}`}
    >
      {children}
    </span>
  );
}

export function UsageBar({ label, percent }: { label: string; percent: number }) {
  const color = percent >= 85 ? "bg-bad" : percent >= 70 ? "bg-warn" : "bg-ok";
  return (
    <div>
      <div className="mb-1 flex justify-between text-xs text-muted">
        <span>{label}</span>
        <span>{percent}%</span>
      </div>
      <div
        className="h-2 overflow-hidden rounded-full bg-surface-2"
        role="progressbar"
        aria-label={label}
        aria-valuenow={percent}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div className={`h-full rounded-full ${color}`} style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}

export function StatCard({
  label,
  value,
  percent,
  hint,
}: {
  label: string;
  value: string;
  percent?: number;
  hint?: string;
}) {
  return (
    <Card>
      <p className="text-xs text-muted">{label}</p>
      <p className="mt-1 text-2xl font-semibold tabular-nums">{value}</p>
      {percent !== undefined && (
        <div className="mt-3">
          <UsageBar label={label} percent={percent} />
        </div>
      )}
      {hint && <p className="mt-2 text-xs text-muted">{hint}</p>}
    </Card>
  );
}

export type TableHeader = string | { label: string; className?: string };

/** Pass `className: "hidden lg:table-cell"` on a header (and the same on its cells) to drop a column on narrow windows. */
export function Table({ headers, children }: { headers: TableHeader[]; children: ReactNode }) {
  return (
    <div className="overflow-x-auto rounded-md border border-border bg-surface">
      <table className="w-full text-left text-sm">
        <thead className="sticky top-0 border-b border-border bg-surface-2 text-xs uppercase tracking-wide text-muted">
          <tr>
            {headers.map((h, i) => {
              const { label, className = "" } = typeof h === "string" ? { label: h } : h;
              return (
                <th
                  key={`${label}-${i}`}
                  className={`whitespace-nowrap px-3 py-2 font-medium ${className}`}
                >
                  {label}
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody className="divide-y divide-border">{children}</tbody>
      </table>
    </div>
  );
}
