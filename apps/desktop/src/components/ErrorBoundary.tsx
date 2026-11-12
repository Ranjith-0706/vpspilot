import { Component, type ReactNode } from "react";

/**
 * Last-resort screen shown when something throws while rendering. It deliberately shows no error
 * message or stack: those can contain server names, hosts or other data the user did not expect
 * to see (or share in a screenshot). It does not depend on any app provider or component, so it
 * still works when those are what failed.
 */
export function ErrorFallback({ onReload }: { onReload: () => void }) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-bg p-6 text-fg">
      <div
        role="alert"
        className="w-full max-w-md rounded-md border border-border bg-surface p-6 text-center"
      >
        <h1 className="text-lg font-semibold">Something went wrong</h1>
        <p className="mt-2 text-sm text-muted">
          VPSPilot hit an unexpected problem and had to stop showing this screen. Your saved servers
          and settings are not affected. Reloading restarts the interface and closes any open
          terminal sessions.
        </p>
        <button
          type="button"
          onClick={onReload}
          className="mt-5 rounded-md bg-accent px-4 py-2 text-sm font-medium text-white outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2"
        >
          Reload VPSPilot
        </button>
      </div>
    </div>
  );
}

interface Props {
  children: ReactNode;
  /** Defaults to reloading the page. Overridable for tests. */
  onReload?: () => void;
}

interface State {
  failed: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  override state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  override componentDidCatch(error: Error): void {
    // Only the error's type is logged. Its message and stack may contain user data.
    console.error(`VPSPilot interface error (${error.name})`);
  }

  private reload = (): void => {
    (this.props.onReload ?? (() => window.location.reload()))();
  };

  override render(): ReactNode {
    return this.state.failed ? <ErrorFallback onReload={this.reload} /> : this.props.children;
  }
}
