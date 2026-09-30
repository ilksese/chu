import { useLayoutEffect, useRef, type ComponentPropsWithoutRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useBodyScrollLock } from "@/lib/hooks/useBodyScrollLock";

type DialogProps = Omit<ComponentPropsWithoutRef<"dialog">, "onClose"> & {
  children: ReactNode;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
};

export function Dialog({ children, className, onClose, returnFocus, ...props }: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  useBodyScrollLock();

  useLayoutEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    const initialFocus =
      dialog?.querySelector<HTMLElement>(
        "input:not([type='hidden']):not([disabled]), select:not([disabled]), textarea:not([disabled])",
      ) ??
      dialog?.querySelector<HTMLElement>(
        "button:not([disabled]), [href], [tabindex]:not([tabindex='-1'])",
      );
    initialFocus?.focus({ preventScroll: true });
    return () => {
      if (dialog?.open) dialog.close();
      returnFocus?.focus();
    };
  }, [returnFocus]);

  return createPortal(
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
    </dialog>,
    document.body,
  );
}
