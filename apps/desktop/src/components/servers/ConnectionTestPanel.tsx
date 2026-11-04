import { useState } from "react";
import { Badge } from "../ui";
import { Button } from "../Field";
import { ConfirmDialog } from "../ConfirmDialog";
import { serversApi, toAppError } from "../../services/serversApi";
import { canTrustHostKey, isHostKeyMismatch, outcomeView } from "../../services/connectionTest";
import type { ConnectionTestResult, HostKeyRecord, ServerProfile } from "../../types/servers";

const BOX: Record<string, string> = {
  ok: "border-ok/40 bg-ok/10",
  warn: "border-warn/40 bg-warn/10",
  bad: "border-bad/40 bg-bad/10",
  neutral: "border-border bg-surface-2",
};

type Busy = "test" | "trust" | "forget" | null;

function Fingerprint({ label, value }: { label: string; value: string }) {
  return (
    <div className="mt-2">
      <p className="text-xs text-muted">{label}</p>
      <p className="break-all font-mono text-xs">{value}</p>
    </div>
  );
}

export function ConnectionTestPanel({
  server,
  hostKey,
  result,
  onResult,
  onChanged,
}: {
  server: ServerProfile;
  hostKey: HostKeyRecord | undefined;
  result: ConnectionTestResult | undefined;
  onResult: (result: ConnectionTestResult | undefined) => void;
  onChanged: () => Promise<void>;
}) {
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmForget, setConfirmForget] = useState(false);

  async function runTest() {
    setBusy("test");
    setError(null);
    try {
      onResult(await serversApi.testConnection(server.id));
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(null);
    }
  }

  async function trust() {
    if (!result?.hostKey) return;
    setBusy("trust");
    setError(null);
    try {
      await serversApi.trustHostKey(server.id, result.hostKey.fingerprint);
      await onChanged();
    } catch (err) {
      setError(toAppError(err).message);
      setBusy(null);
      return;
    }
    await runTest(); // re-test now that the key is trusted
  }

  async function forget() {
    setBusy("forget");
    setError(null);
    try {
      await serversApi.forgetHostKey(server.id);
      onResult(undefined);
      await onChanged();
      setConfirmForget(false);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(null);
    }
  }

  const view = result ? outcomeView(result.outcome) : null;

  return (
    <div className="mt-5 border-t border-border pt-4">
      <h3 className="text-sm font-semibold">Connection test</h3>
      <p className="mt-1 text-xs text-muted">
        Opens an SSH login to check the host key and credential, then disconnects immediately. It
        does not run any commands on the server.
      </p>

      <div className="mt-3 text-sm">
        <p className="text-xs text-muted">Trusted host key</p>
        {hostKey ? (
          <>
            <Badge tone="ok">Trusted</Badge>
            <Fingerprint label={hostKey.keyType} value={hostKey.fingerprint} />
            <p className="mt-1 text-xs text-muted">
              Trusted {new Date(hostKey.trustedAt).toLocaleString()}
            </p>
            <button
              type="button"
              onClick={() => setConfirmForget(true)}
              className="mt-1 text-xs text-bad underline"
            >
              Forget this host key…
            </button>
          </>
        ) : (
          <Badge tone="warn">Not trusted yet</Badge>
        )}
      </div>

      <Button className="mt-4" onClick={() => void runTest()} disabled={busy !== null}>
        {busy === "test" ? "Testing…" : "Test connection"}
      </Button>

      {error && (
        <p role="alert" className="mt-3 rounded-lg bg-bad/10 p-3 text-sm text-bad">
          {error}
        </p>
      )}

      {result && view && (
        <div role="status" className={`mt-4 rounded-lg border p-3 text-sm ${BOX[view.tone]}`}>
          <div className="flex items-center justify-between gap-2">
            <Badge tone={view.tone}>{view.label}</Badge>
            <span className="text-xs text-muted">
              {new Date(result.testedAt).toLocaleTimeString()}
              {result.latencyMs !== undefined ? ` · ${result.latencyMs} ms` : ""}
            </span>
          </div>
          <p className="mt-2">{result.message}</p>

          {isHostKeyMismatch(result) && (
            <>
              {result.expectedFingerprint && (
                <Fingerprint label="Trusted fingerprint" value={result.expectedFingerprint} />
              )}
              {result.hostKey && (
                <Fingerprint label="Fingerprint presented now" value={result.hostKey.fingerprint} />
              )}
              <p className="mt-2 text-xs">
                Only continue if you know the server was rebuilt or its keys were rotated. Then
                forget the old key and verify the new fingerprint with your provider.
              </p>
            </>
          )}

          {canTrustHostKey(result) && result.hostKey && (
            <>
              <Fingerprint
                label={`Host key (${result.hostKey.keyType})`}
                value={result.hostKey.fingerprint}
              />
              <p className="mt-2 text-xs">
                Compare this with your server's real fingerprint (for example by running{" "}
                <span className="font-mono">ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</span>{" "}
                on it, or checking your provider's console). Only trust it if it matches exactly.
              </p>
              <Button
                className="mt-3"
                variant="primary"
                onClick={() => void trust()}
                disabled={busy !== null}
              >
                {busy === "trust" ? "Trusting…" : "Trust this host key and retest"}
              </Button>
            </>
          )}
        </div>
      )}

      {confirmForget && (
        <ConfirmDialog
          title="Forget host key?"
          confirmLabel="Forget key"
          busy={busy === "forget"}
          error={error}
          onConfirm={() => void forget()}
          onCancel={() => {
            setConfirmForget(false);
            setError(null);
          }}
        >
          <p>
            VPSPilot will stop trusting the stored key for {server.name}. The next test will show
            the server's current fingerprint for you to verify again.
          </p>
        </ConfirmDialog>
      )}
    </div>
  );
}
