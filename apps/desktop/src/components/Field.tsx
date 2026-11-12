import type { ReactNode } from "react";

export const inputClass =
  "w-full rounded-md border border-border bg-bg px-2.5 py-1.5 text-sm outline-none transition-colors focus:border-accent focus:ring-1 focus:ring-accent aria-[invalid=true]:border-bad";

export function Field({
  id,
  label,
  error,
  hint,
  children,
}: {
  id: string;
  label: string;
  error?: string | undefined;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div>
      <label htmlFor={id} className="mb-1 block text-xs font-medium text-muted">
        {label}
      </label>
      {children}
      {hint && !error && <p className="mt-1 text-xs text-muted">{hint}</p>}
      {error && (
        <p id={`${id}-error`} role="alert" className="mt-1 text-xs text-bad">
          {error}
        </p>
      )}
    </div>
  );
}

export function Button({
  variant = "secondary",
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "danger";
}) {
  const styles = {
    primary: "bg-accent text-white hover:brightness-110",
    secondary: "border border-border bg-surface text-fg hover:bg-surface-2",
    danger: "bg-bad text-white hover:brightness-110",
  }[variant];
  return (
    <button
      type="button"
      {...props}
      className={`rounded-md px-3 py-1.5 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-50 ${styles} ${props.className ?? ""}`}
    />
  );
}
