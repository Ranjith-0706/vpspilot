import { Button } from "./Field";
import { Dialog, DialogBody, DialogFooter } from "./Dialog";

export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  busy,
  error,
  onConfirm,
  onCancel,
}: {
  title: string;
  children: React.ReactNode;
  confirmLabel: string;
  busy: boolean;
  error: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <Dialog title={title} onClose={onCancel}>
      <DialogBody>
        <div className="text-sm">{children}</div>
        {error && (
          <p role="alert" className="rounded-md bg-bad/10 p-3 text-sm text-bad">
            {error}
          </p>
        )}
      </DialogBody>
      <DialogFooter>
        <Button onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button variant="danger" onClick={onConfirm} disabled={busy}>
          {busy ? "Working…" : confirmLabel}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
