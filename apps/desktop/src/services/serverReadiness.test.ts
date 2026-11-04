import { describe, expect, it } from "vitest";
import { connectionBlocker } from "./serverReadiness";
import type { HostKeyRecord } from "../types/servers";

const key: HostKeyRecord = {
  serverId: "s",
  keyType: "ssh-ed25519",
  fingerprint: "SHA256:abc",
  trustedAt: 0,
};

describe("connectionBlocker", () => {
  it("requires a trusted host key first", () => {
    expect(connectionBlocker({ credentialRef: "ref" }, undefined)).toMatch(/host key/);
    expect(connectionBlocker({ credentialRef: null }, undefined)).toMatch(/host key/);
  });

  it("then requires a stored credential", () => {
    expect(connectionBlocker({ credentialRef: null }, key)).toMatch(/credential/);
  });

  it("allows a server with both", () => {
    expect(connectionBlocker({ credentialRef: "vpspilot:server:s" }, key)).toBeNull();
  });
});
