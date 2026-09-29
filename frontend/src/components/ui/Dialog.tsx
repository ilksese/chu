import { useEffect, useRef, type ComponentPropsWithoutRef, type ReactNode } from "react";
import { useBodyScrollLock } from "@/lib/hooks/useBodyScrollLock";

type DialogProps = Omit<ComponentPropsWithoutRef<"dialog">, "onClose"> & {
  children: ReactNode;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
};

export function Dialog({ children, className, onClose, returnFocus, ...props }: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  useBodyScrollLock();

  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => {
      if (dialog?.open) dialog.close();
      returnFocus?.focus();
    };
  }, [returnFocus]);

  return (
    <dialog
      ref={ref}
      className={`m-auto max-h-[calc(100vh-48px)] max-w-[calc(100vw-48px)] overflow-auto rounded-lg border-2 border-border bg-card p-0 text-card-foreground shadow-neo-lg backdrop:bg-foreground/58 ${className ?? ""}`}
      {...props}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      {children}
    </dialog>
  );
}
