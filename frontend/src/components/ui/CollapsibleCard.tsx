import { useId, useState, type ComponentProps, type ReactNode } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { ChevronDown } from "lucide-react";
import { tv } from "tailwind-variants";

const collapsibleCard = tv({
  base: "overflow-hidden rounded-lg border-2 border-border bg-card shadow-neo-sm",
});

type CollapsibleCardProps = ComponentProps<"article"> & {
  summary: ReactNode;
  actions?: ReactNode;
  header?: ReactNode;
};

export function CollapsibleCard({
  summary,
  actions,
  header,
  children,
  className,
  ...props
}: CollapsibleCardProps) {
  const [open, setOpen] = useState(false);
  const reducedMotion = useReducedMotion();
  const contentID = useId();

  return (
    <article className={collapsibleCard({ className })} {...props}>
      <div className="flex min-h-[78px] items-center gap-3 px-4 py-3">
        {header ?? (
          <button
            type="button"
            className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 border-0 bg-transparent p-0 text-left"
            aria-expanded={open}
            aria-controls={contentID}
            onClick={() => setOpen((current) => !current)}
          >
            {summary}
            <ChevronDown
              className={`size-4 shrink-0 transition-transform ${open ? "rotate-180" : ""}`}
            />
          </button>
        )}
        {actions}
      </div>

      <AnimatePresence initial={false}>
        {open ? (
          <motion.div
            id={contentID}
            className="overflow-hidden"
            initial={reducedMotion ? false : { height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={
              reducedMotion ? { duration: 0 } : { duration: 0.18, ease: [0.16, 1, 0.3, 1] }
            }
          >
            <div className="border-t-2 border-border">{children}</div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </article>
  );
}
