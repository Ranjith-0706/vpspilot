import { useEffect, useRef, type ReactNode } from "react";

/**
 * Modal built on the native <dialog>: focus trapping and Escape handling come from the browser.
 *
 * Layout: a fixed title bar, a scrolling body and a fixed footer, so the action buttons are
 * always visible however small the window is. Forms put `DialogBody` and `DialogFooter` inside
 * a `<form className={DIALOG_FORM}>`; simple dialogs can pass plain children.
 */
export function Dialog({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, []);

  return (
    <dialog
      ref={ref}
      aria-labelledby="dialog-title"
      onClose={onClose}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      className="m-auto w-[calc(100vw-1.5rem)] max-w-lg overflow-hidden rounded-lg border border-border bg-surface p-0 text-fg shadow-2xl backdrop:bg-black/60 open:flex"
    >
      <div className="flex max-h-[calc(100dvh-1.5rem)] min-h-0 w-full flex-col">
        <h2
          id="dialog-title"
          className="shrink-0 border-b border-border bg-surface-2 px-5 py-3 text-sm font-semibold"
        >
          {title}
        </h2>
        {children}
      </div>
    </dialog>
  );
}

/** Class for a `<form>` that contains a DialogBody and a DialogFooter. */
export const DIALOG_FORM = "flex min-h-0 flex-1 flex-col";

export function DialogBody({ children }: { children: ReactNode }) {
  return <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-5 py-4">{children}</div>;
}

export function DialogFooter({ children }: { children: ReactNode }) {
  return (
    <div className="flex shrink-0 justify-end gap-2 border-t border-border bg-surface-2 px-5 py-3">
      {children}
    </div>
  );
}
