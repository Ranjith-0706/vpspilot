// Mirrors the Rust validation in src-tauri/src/servers/model.rs. The backend is the source
// of truth; this exists for fast feedback. Keep the limits in sync.
import type { ServerFormErrors, ServerFormValues, ServerInput } from "../types/servers";

export const LIMITS = {
  nameMax: 64,
  usernameMax: 32,
  descriptionMax: 500,
  secretMaxBytes: 2560,
} as const;

export const DEFAULT_SSH_PORT = "22";

const IPV4 = /^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$/;
const HOST_LABEL = /^[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?$/;
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

/** Counts 16-bit groups in one side of an IPv6 address, or null if invalid. */
function countGroups(part: string, allowV4Tail: boolean): number | null {
  if (part === "") return 0;
  const groups = part.split(":");
  let n = 0;
  for (const [i, g] of groups.entries()) {
    if (allowV4Tail && i === groups.length - 1 && g.includes(".")) {
      if (!IPV4.test(g)) return null;
      n += 2;
    } else if (/^[0-9A-Fa-f]{1,4}$/.test(g)) n += 1;
    else return null;
  }
  return n;
}

function isIPv6(s: string): boolean {
  if (!s.includes(":") || !/^[0-9A-Fa-f:.]+$/.test(s)) return false;
  const parts = s.split("::");
  if (parts.length > 2) return false;
  if (parts.length === 1) return countGroups(s, true) === 8;
  const left = countGroups(parts[0] ?? "", false);
  const right = countGroups(parts[1] ?? "", true);
  return left !== null && right !== null && left + right <= 7;
}

export function isValidHost(host: string): boolean {
  if (IPV4.test(host) || isIPv6(host)) return true;
  if (host.length === 0 || host.length > 253) return false;
  const labels = host.split(".");
  if (!labels.every((l) => l.length <= 63 && HOST_LABEL.test(l))) return false;
  return !/^\d+$/.test(labels[labels.length - 1] ?? ""); // "999.1.1.1" is a bad IP, not a hostname
}

const utf8Length = (s: string) => new TextEncoder().encode(s).length;

export function validateSecret(secret: string): string | undefined {
  if (secret.trim() === "") return "Credential must not be empty.";
  if (utf8Length(secret) > LIMITS.secretMaxBytes)
    return `Credential is too large for the OS credential store (max ${LIMITS.secretMaxBytes} bytes). For SSH keys, ed25519, ECDSA and RSA-2048 keys fit; RSA-3072 and RSA-4096 keys are usually too large.`;
  return undefined;
}

export interface ValidationResult {
  errors: ServerFormErrors;
  /** Present only when `errors` is empty. */
  input?: ServerInput;
  /** Present when the user entered a credential and it is valid. */
  secret?: string;
}

export function validateServerForm(v: ServerFormValues): ValidationResult {
  const errors: ServerFormErrors = {};

  const name = v.name.trim();
  if (!name) errors.name = "Server name is required.";
  else if ([...name].length > LIMITS.nameMax || CONTROL.test(name))
    errors.name = `Server name must be at most ${LIMITS.nameMax} characters.`;

  const host = v.host.trim();
  if (!host) errors.host = "Hostname or IP address is required.";
  else if (!isValidHost(host)) errors.host = "Enter a valid hostname or IP address.";

  const portText = v.port.trim();
  const port = Number(portText);
  if (!/^\d+$/.test(portText) || port < 1 || port > 65535)
    errors.port = "SSH port must be a whole number between 1 and 65535.";

  const username = v.username.trim();
  if (!username) errors.username = "SSH username is required.";
  else if (
    [...username].length > LIMITS.usernameMax ||
    username.startsWith("-") ||
    !/^[A-Za-z0-9._-]+$/.test(username)
  )
    errors.username = `Username must be at most ${LIMITS.usernameMax} characters using letters, digits, '.', '_' or '-'.`;

  const description = v.description.trim();
  if ([...description].length > LIMITS.descriptionMax)
    errors.description = `Description must be at most ${LIMITS.descriptionMax} characters.`;

  let secret: string | undefined;
  if (v.secret !== "") {
    const secretError = validateSecret(v.secret);
    if (secretError) errors.secret = secretError;
    else secret = v.secret;
  }

  if (Object.keys(errors).length > 0) return { errors };

  const input: ServerInput = {
    name,
    host,
    port,
    username,
    authMethod: v.authMethod,
    ...(description ? { description } : {}),
  };
  return secret === undefined ? { errors, input } : { errors, input, secret };
}
