export type AuthMethod = "ssh_key" | "password";

/** A saved server profile. Holds only a credential *reference*, never a secret. */
export interface ServerProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authMethod: AuthMethod;
  credentialRef: string | null;
  description: string | null;
  /** Unix epoch milliseconds. */
  createdAt: number;
  updatedAt: number;
}

/** Payload sent to the Rust backend (secrets are sent separately and never stored in the UI). */
export interface ServerInput {
  name: string;
  host: string;
  port: number;
  username: string;
  authMethod: AuthMethod;
  description?: string;
}

/** Raw form state. `port` stays a string while the user types. */
export interface ServerFormValues {
  name: string;
  host: string;
  port: string;
  username: string;
  authMethod: AuthMethod;
  description: string;
  secret: string;
}

export type ServerFormField = keyof ServerFormValues;
export type ServerFormErrors = Partial<Record<ServerFormField, string>>;

export type AppErrorCode = "validation" | "not_found" | "database" | "credential" | "internal";

export interface AppError {
  code: AppErrorCode;
  message: string;
  field?: string;
}

/** Real connectivity is not implemented yet, so every profile is "not_connected". */
export type ConnectionState = "not_connected";
