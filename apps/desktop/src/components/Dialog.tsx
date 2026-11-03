import { useEffect, useRef, type ReactNode } from "react";

/** Modal built on the native <dialog>: focus trapping and Escape handling come from the browser. */
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
      className="m-auto w-full max-w-lg rounded-xl border border-border bg-surface p-0 text-fg backdrop:bg-black/50"
    >
      <div className="max-h-[85vh] overflow-y-auto p-6">
        <h2 id="dialog-title" className="mb-4 text-lg font-semibold">
          {title}
        </h2>
        {children}
      </div>
    </dialog>
  );
}
