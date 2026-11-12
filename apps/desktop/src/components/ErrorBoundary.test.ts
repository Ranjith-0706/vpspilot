import { createElement, isValidElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ErrorBoundary, ErrorFallback } from "./ErrorBoundary";

// The test environment has no DOM, so the boundary is exercised through React's own contract:
// a failed render calls getDerivedStateFromError, and the next render returns the fallback.
function failedBoundary(onReload: () => void): ErrorBoundary {
  const boundary = new ErrorBoundary({ children: createElement("main", null, "app"), onReload });
  boundary.state = ErrorBoundary.getDerivedStateFromError();
  return boundary;
}

afterEach(() => vi.restoreAllMocks());

describe("ErrorBoundary", () => {
  it("renders its children when nothing has failed", () => {
    const child = createElement("main", null, "app");
    const boundary = new ErrorBoundary({ children: child });
    expect(boundary.render()).toBe(child);
  });

  it("switches to the fallback after a render error", () => {
    expect(ErrorBoundary.getDerivedStateFromError()).toEqual({ failed: true });
    const out = failedBoundary(() => undefined).render();
    expect(isValidElement(out)).toBe(true);
    expect((out as ReactElement).type).toBe(ErrorFallback);
  });

  it("shows a recovery screen with a reload button and no error details", () => {
    const html = renderToStaticMarkup(failedBoundary(() => undefined).render() as ReactElement);
    expect(html).toContain("Something went wrong");
    expect(html).toContain("Reload VPSPilot");
    expect(html).toContain('role="alert"');
  });

  it("runs the reload action when the button is used", () => {
    const reload = vi.fn();
    const out = failedBoundary(reload).render() as ReactElement<{ onReload: () => void }>;
    out.props.onReload();
    expect(reload).toHaveBeenCalledTimes(1);
  });

  it("logs only the error type, never its message or stack", () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const secret = "db-password=hunter2 host=10.0.0.5";
    const error = new TypeError(secret);
    failedBoundary(() => undefined).componentDidCatch(error);
    const logged = JSON.stringify(log.mock.calls);
    expect(logged).toContain("TypeError");
    expect(logged).not.toContain("hunter2");
    expect(logged).not.toContain("10.0.0.5");
  });
});
