import { describe, expect, it } from "vitest";
import { LIMITS, isValidHost, validateSecret, validateServerForm } from "./serverValidation";
import type { ServerFormValues } from "../types/servers";

const valid: ServerFormValues = {
  name: "web-01",
  host: "web-01.example",
  port: "22",
  username: "deploy",
  authMethod: "ssh_key",
  description: "",
  secret: "",
};

const withValues = (patch: Partial<ServerFormValues>): ServerFormValues => ({ ...valid, ...patch });

describe("validateServerForm", () => {
  it("accepts valid input, trims it and omits an empty description", () => {
    const r = validateServerForm(withValues({ name: "  web-01 ", description: "  " }));
    expect(r.errors).toEqual({});
    expect(r.input).toEqual({
      name: "web-01",
      host: "web-01.example",
      port: 22,
      username: "deploy",
      authMethod: "ssh_key",
    });
    expect(r.secret).toBeUndefined();
  });

  it("keeps a description and a provided secret", () => {
    const r = validateServerForm(withValues({ description: " prod ", secret: "pw" }));
    expect(r.input?.description).toBe("prod");
    expect(r.secret).toBe("pw");
  });

  it("requires name, host and username", () => {
    const r = validateServerForm(withValues({ name: " ", host: "", username: "" }));
    expect(Object.keys(r.errors).sort()).toEqual(["host", "name", "username"]);
    expect(r.input).toBeUndefined();
  });

  it.each(["0", "65536", "-1", "22.5", "abc", "", "1e3"])("rejects port %j", (port) => {
    expect(validateServerForm(withValues({ port })).errors.port).toBeDefined();
  });

  it.each(["1", "22", "65535"])("accepts port %j", (port) => {
    expect(validateServerForm(withValues({ port })).errors.port).toBeUndefined();
  });

  it("enforces length limits", () => {
    expect(
      validateServerForm(withValues({ name: "x".repeat(LIMITS.nameMax + 1) })).errors.name,
    ).toBeDefined();
    expect(
      validateServerForm(withValues({ username: "u".repeat(LIMITS.usernameMax + 1) })).errors
        .username,
    ).toBeDefined();
    expect(
      validateServerForm(withValues({ description: "d".repeat(LIMITS.descriptionMax + 1) })).errors
        .description,
    ).toBeDefined();
  });

  it.each(["-oProxyCommand=x", "a b", "root;id"])("rejects username %j", (username) => {
    expect(validateServerForm(withValues({ username })).errors.username).toBeDefined();
  });

  it("tells the user which key types fit when a credential is too large", () => {
    const message = validateSecret("x".repeat(LIMITS.secretMaxBytes + 1));
    expect(message).toContain(String(LIMITS.secretMaxBytes));
    expect(message).toMatch(/ed25519/);
    expect(message).toMatch(/RSA-4096/);
    expect(validateSecret("x".repeat(LIMITS.secretMaxBytes))).toBeUndefined();
  });

  it("validates a provided secret", () => {
    expect(validateServerForm(withValues({ secret: "   " })).errors.secret).toBeDefined();
    expect(
      validateServerForm(withValues({ secret: "x".repeat(LIMITS.secretMaxBytes + 1) })).errors
        .secret,
    ).toBeDefined();
  });
});

describe("isValidHost", () => {
  it.each([
    "10.0.0.5",
    "255.255.255.255",
    "2001:db8::1",
    "::1",
    "::",
    "::ffff:1.2.3.4",
    "localhost",
    "a-b.example.com",
  ])("accepts %j", (h) => expect(isValidHost(h)).toBe(true));

  it.each([
    "",
    "bad host",
    "-a.com",
    "a-.com",
    "a..com",
    "999.1.1.1",
    "1.2.3",
    "a_b.com",
    "[::1]",
    "x;rm -rf",
    "1:2:3:4:5:6:7:8:9",
    "1::2::3",
    ":::",
    "12345::1",
    "http://host",
  ])("rejects %j", (h) => expect(isValidHost(h)).toBe(false));
});

describe("validateSecret", () => {
  it("accepts normal secrets and rejects empty or oversized ones", () => {
    expect(validateSecret("hunter2")).toBeUndefined();
    expect(validateSecret("")).toBeDefined();
    expect(validateSecret("é".repeat(LIMITS.secretMaxBytes))).toBeDefined(); // bytes, not chars
  });
});
