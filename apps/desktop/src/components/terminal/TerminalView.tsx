import { useEffect, useRef, useState } from "react";
import { Terminal, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { Badge } from "../ui";
import { Button } from "../Field";
import {
  closeTerminal,
  openTerminal,
  resizeTerminal,
  writeTerminal,
} from "../../services/terminalApi";
import { toAppError } from "../../services/ipc";
import { base64ToBytes, chunkInput } from "../../services/terminalData";

type Status =
  | { kind: "connecting" }
  | { kind: "connected" }
  | { kind: "closed"; reason?: string | undefined }
  | { kind: "error"; message: string };

/** Reads the app's current theme colors so the terminal matches light/dark mode. */
function readTheme(): ITheme {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return {
    background: v("--bg"),
    foreground: v("--fg"),
    cursor: v("--accent"),
    selectionBackground: v("--border"),
  };
}

export function TerminalView({
  serverId,
  title,
  onDisconnect,
  onReconnect,
}: {
  serverId: string;
  title: string;
  onDisconnect: () => void;
  onReconnect: () => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<Status>({ kind: "connecting" });

  useEffect(() => {
    const host = container.current;
    if (!host) return;

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: '"Cascadia Mono", Consolas, "Courier New", monospace',
      fontSize: 14,
      scrollback: 5000,
      theme: readTheme(),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    fit.fit();

    let sessionId: string | null = null;
    let disposed = false; // React StrictMode mounts effects twice in development
    let pending: Promise<void> = Promise.resolve(); // keeps keystrokes and paste chunks in order

    openTerminal(serverId, term.cols, term.rows, (event) => {
      if (disposed) return;
      if (event.type === "data") term.write(base64ToBytes(event.data));
      else setStatus({ kind: "closed", reason: event.reason });
    })
      .then((id) => {
        if (disposed) {
          void closeTerminal(id).catch(() => undefined);
          return;
        }
        sessionId = id;
        setStatus((s) => (s.kind === "connecting" ? { kind: "connected" } : s));
        term.focus();
      })
      .catch((err: unknown) => {
        if (!disposed) setStatus({ kind: "error", message: toAppError(err).message });
      });

    const input = term.onData((data) => {
      const id = sessionId;
      if (!id) return;
      pending = pending.then(async () => {
        try {
          for (const chunk of chunkInput(data)) await writeTerminal(id, chunk);
        } catch {
          /* the session ended; the "closed" event updates the UI */
        }
      });
    });
    const resized = term.onResize(({ cols, rows }) => {
      if (sessionId) void resizeTerminal(sessionId, cols, rows).catch(() => undefined);
    });
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(host);
    const themeWatcher = new MutationObserver(() => {
      term.options.theme = readTheme();
    });
    themeWatcher.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => {
      disposed = true;
      observer.disconnect();
      themeWatcher.disconnect();
      input.dispose();
      resized.dispose();
      if (sessionId) void closeTerminal(sessionId).catch(() => undefined);
      term.dispose();
    };
  }, [serverId]);

  const ended = status.kind === "closed" || status.kind === "error";

  return (
    <div>
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <span className="font-medium">{title}</span>
          {status.kind === "connecting" && <Badge tone="warn">Connecting…</Badge>}
          {status.kind === "connected" && <Badge tone="ok">Connected</Badge>}
          {status.kind === "closed" && <Badge tone="neutral">Session ended</Badge>}
          {status.kind === "error" && <Badge tone="bad">Could not connect</Badge>}
        </div>
        <div className="flex gap-2">
          {ended && <Button onClick={onReconnect}>Reconnect</Button>}
          <Button variant={ended ? "secondary" : "danger"} onClick={onDisconnect}>
            {ended ? "Close" : "Disconnect"}
          </Button>
        </div>
      </div>

      {status.kind === "error" && (
        <p role="alert" className="mb-3 rounded-md bg-bad/10 p-3 text-sm text-bad">
          {status.message}
        </p>
      )}
      {status.kind === "closed" && status.reason && (
        <p role="status" className="mb-3 rounded-md bg-surface-2 p-3 text-sm text-muted">
          {status.reason}
        </p>
      )}

      <div
        ref={container}
        aria-label="Terminal"
        className="h-[calc(100dvh-14rem)] min-h-64 overflow-hidden rounded-md border border-border bg-bg p-2"
      />
    </div>
  );
}
