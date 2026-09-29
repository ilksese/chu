import {
  useLayoutEffect,
  useRef,
  type CSSProperties,
  type ReactNode,
  type Ref,
  type ToggleEvent,
} from "react";
import { tv } from "tailwind-variants";

const popover = tv({
  base: "fixed inset-auto z-80 m-0 w-[min(240px,calc(100vw-24px))] rounded-lg border-2 border-border bg-card p-3 text-card-foreground shadow-neo",
});

function Popover({
  open,
  anchor,
  onClose,
  className,
  children,
}: {
  open: boolean;
  anchor: Ref<HTMLElement>;
  onClose: () => void;
  className?: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const target = anchor && typeof anchor !== "function" ? anchor.current : null;

  useLayoutEffect(() => {
    const pop = ref.current;
    if (!open || !target || !pop) return;
    if (!pop.matches(":popover-open")) pop.showPopover();
    const position = () => {
      const anchorRect = target.getBoundingClientRect();
      const popRect = pop.getBoundingClientRect();
      const edge = 12;
      const gap = 8;
      const top =
        anchorRect.top - popRect.height - gap >= edge
          ? anchorRect.top - popRect.height - gap
          : anchorRect.bottom + gap;
      pop.style.left = `${Math.max(edge, Math.min(anchorRect.right - popRect.width, window.innerWidth - popRect.width - edge))}px`;
      pop.style.top = `${Math.max(edge, Math.min(top, window.innerHeight - popRect.height - edge))}px`;
    };
    position();
    window.addEventListener("resize", position);
    window.addEventListener("scroll", position, true);
    return () => {
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", position, true);
      if (pop.matches(":popover-open")) pop.hidePopover();
    };
  }, [open, target]);

  return (
    <div
      ref={ref}
      popover="auto"
      className={popover({ className })}
      style={{ visibility: open ? "visible" : "hidden" } as CSSProperties}
      onToggle={(event: ToggleEvent<HTMLDivElement>) => {
        if (event.newState === "closed" && open) onClose();
      }}
    >
      {children}
    </div>
  );
}

export { Popover };
