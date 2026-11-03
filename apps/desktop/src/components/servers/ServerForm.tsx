import { useState, type FormEvent } from "react";
import { Button, Field, inputClass } from "../Field";
import { DEFAULT_SSH_PORT, LIMITS, validateServerForm } from "../../services/serverValidation";
import { toAppError } from "../../services/serversApi";
import type {
  AuthMethod,
  ServerFormErrors,
  ServerFormField,
  ServerFormValues,
  ServerInput,
  ServerProfile,
} from "../../types/servers";

export interface ServerFormSubmit {
  input: ServerInput;
  /** Present only if the user typed a credential. Never persisted by the UI. */
  secret?: string;
  removeCredential: boolean;
}

const FIELDS: ServerFormField[] = [
  "name",
  "host",
  "port",
  "username",
  "authMethod",
  "description",
  "secret",
];

function initialValues(server?: ServerProfile): ServerFormValues {
  return {
    name: server?.name ?? "",
    host: server?.host ?? "",
    port: server ? String(server.port) : DEFAULT_SSH_PORT,
    username: server?.username ?? "",
    authMethod: server?.authMethod ?? "ssh_key",
    description: server?.description ?? "",
    secret: "",
  };
}

export function ServerForm({
  server,
  onSubmit,
  onCancel,
}: {
  server?: ServerProfile | undefined;
  onSubmit: (data: ServerFormSubmit) => Promise<void>;
  onCancel: () => void;
}) {
  const [values, setValues] = useState<ServerFormValues>(() => initialValues(server));
  const [errors, setErrors] = useState<ServerFormErrors>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [removeCredential, setRemoveCredential] = useState(false);
  const [busy, setBusy] = useState(false);

  const hasCredential = Boolean(server?.credentialRef);
  const methodChanged = server !== undefined && server.authMethod !== values.authMethod;
  const isKey = values.authMethod === "ssh_key";

  const set = <K extends ServerFormField>(key: K, value: ServerFormValues[K]) =>
    setValues((v) => ({ ...v, [key]: value }));

  const props = (key: ServerFormField) => ({
    id: `server-${key}`,
    "aria-invalid": errors[key] ? true : undefined,
    "aria-describedby": errors[key] ? `server-${key}-error` : undefined,
  });

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setFormError(null);
    const result = validateServerForm(values);
    setErrors(result.errors);
    if (!result.input) return;

    setBusy(true);
    try {
      await onSubmit({
        input: result.input,
        ...(result.secret !== undefined ? { secret: result.secret } : {}),
        removeCredential,
      });
    } catch (err) {
      const e = toAppError(err);
      if (e.code === "validation" && e.field && (FIELDS as string[]).includes(e.field)) {
        setErrors({ [e.field]: e.message });
      } else {
        setFormError(e.message);
      }
      setBusy(false);
    }
  }

  return (
    <form onSubmit={(e) => void handleSubmit(e)} noValidate className="space-y-4">
      <Field id="server-name" label="Server name" error={errors.name}>
        <input
          {...props("name")}
          className={inputClass}
          value={values.name}
          maxLength={LIMITS.nameMax * 2}
          onChange={(e) => set("name", e.target.value)}
          autoFocus
        />
      </Field>

      <div className="grid grid-cols-[1fr_6rem] gap-3">
        <Field id="server-host" label="Hostname / IP address" error={errors.host}>
          <input
            {...props("host")}
            className={inputClass}
            value={values.host}
            placeholder="server.example.com"
            autoCapitalize="off"
            spellCheck={false}
            onChange={(e) => set("host", e.target.value)}
          />
        </Field>
        <Field id="server-port" label="SSH port" error={errors.port}>
          <input
            {...props("port")}
            className={inputClass}
            value={values.port}
            inputMode="numeric"
            onChange={(e) => set("port", e.target.value)}
          />
        </Field>
      </div>

      <Field id="server-username" label="SSH username" error={errors.username}>
        <input
          {...props("username")}
          className={inputClass}
          value={values.username}
          autoCapitalize="off"
          spellCheck={false}
          onChange={(e) => set("username", e.target.value)}
        />
      </Field>

      <Field id="server-authMethod" label="Authentication method">
        <select
          id="server-authMethod"
          className={inputClass}
          value={values.authMethod}
          onChange={(e) => set("authMethod", e.target.value as AuthMethod)}
        >
          <option value="ssh_key">SSH key</option>
          <option value="password">Password</option>
        </select>
      </Field>

      <Field
        id="server-secret"
        label={isKey ? "Private key (optional)" : "Password (optional)"}
        error={errors.secret}
        hint={
          server && hasCredential
            ? "Leave blank to keep the stored credential. Stored in the OS credential store, never in the app database."
            : "Optional for now. Stored in the OS credential store (Windows Credential Manager), never in the app database."
        }
      >
        {isKey ? (
          <textarea
            {...props("secret")}
            className={`${inputClass} font-mono`}
            rows={4}
            value={values.secret}
            spellCheck={false}
            autoComplete="off"
            placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
            onChange={(e) => set("secret", e.target.value)}
          />
        ) : (
          <input
            {...props("secret")}
            className={inputClass}
            type="password"
            value={values.secret}
            autoComplete="new-password"
            onChange={(e) => set("secret", e.target.value)}
          />
        )}
      </Field>

      {server && hasCredential && !methodChanged && (
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={removeCredential}
            onChange={(e) => setRemoveCredential(e.target.checked)}
          />
          Remove the stored credential
        </label>
      )}
      {methodChanged && hasCredential && (
        <p className="rounded-lg bg-warn/10 p-3 text-xs text-warn">
          Changing the authentication method removes the stored credential. Enter a new one above if
          needed.
        </p>
      )}

      <Field id="server-description" label="Description (optional)" error={errors.description}>
        <textarea
          {...props("description")}
          className={inputClass}
          rows={2}
          value={values.description}
          onChange={(e) => set("description", e.target.value)}
        />
      </Field>

      {formError && (
        <p role="alert" className="rounded-lg bg-bad/10 p-3 text-sm text-bad">
          {formError}
        </p>
      )}

      <div className="flex justify-end gap-2 pt-2">
        <Button onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" disabled={busy}>
          {busy ? "Saving…" : server ? "Save changes" : "Add server"}
        </Button>
      </div>
    </form>
  );
}
