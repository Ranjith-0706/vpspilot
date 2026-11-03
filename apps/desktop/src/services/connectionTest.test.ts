import { describe, expect, it } from "vitest";
import { canTrustHostKey, isHostKeyMismatch, outcomeView } from "./connectionTest";
import type { ConnectionTestOutcome, ConnectionTestResult } from "../types/servers";

const OUTCOMES: ConnectionTestOutcome[] = [
  "success",
  "host_key_unknown",
  "host_key_changed",
  "auth_failed",
  "no_credential",
  "unreachable",
  "timeout",
  "error",
];

const result = (patch: Partial<ConnectionTestResult>): ConnectionTestResult => ({
  outcome: "error",
  message: "",
  testedAt: 0,
  ...patch,
});

describe("outcomeView", () => {
  it.each(OUTCOMES)("has a label for %s", (o) => {
    expect(outcomeView(o).label.length).toBeGreaterThan(0);
  });

  it("only success is shown as ok, and a changed host key is shown as bad", () => {
    expect(outcomeView("success").tone).toBe("ok");
    expect(OUTCOMES.filter((o) => outcomeView(o).tone === "ok")).toEqual(["success"]);
    expect(outcomeView("host_key_changed").tone).toBe("bad");
  });
});

describe("host key trust rules", () => {
  const key = { keyType: "ssh-ed25519", fingerprint: "SHA256:abc" };

  it("allows trusting only an unknown key that was actually presented", () => {
    expect(canTrustHostKey(result({ outcome: "host_key_unknown", hostKey: key }))).toBe(true);
    expect(canTrustHostKey(result({ outcome: "host_key_unknown" }))).toBe(false);
    expect(canTrustHostKey(undefined)).toBe(false);
  });

  it("never allows one-step trust of a changed key", () => {
    const changed = result({
      outcome: "host_key_changed",
      hostKey: key,
      expectedFingerprint: "SHA256:old",
    });
    expect(canTrustHostKey(changed)).toBe(false);
    expect(isHostKeyMismatch(changed)).toBe(true);
  });

  it("does not offer trust after success or failure", () => {
    for (const o of OUTCOMES.filter((o) => o !== "host_key_unknown")) {
      expect(canTrustHostKey(result({ outcome: o, hostKey: key }))).toBe(false);
    }
  });
});
