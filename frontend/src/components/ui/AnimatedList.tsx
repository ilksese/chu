import {
  Children,
  memo,
  useEffect,
  useState,
  type ComponentPropsWithoutRef,
  type ReactNode,
} from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";

export const AnimatedList = memo(function AnimatedList({
  children,
  className,
  delay = 80,
  ...props
}: ComponentPropsWithoutRef<"div"> & { children: ReactNode; delay?: number }) {
  const items = Children.toArray(children);
  const reducedMotion = useReducedMotion();
  const [visible, setVisible] = useState(reducedMotion ? items.length : 0);

  useEffect(() => {
    if (reducedMotion) {
      setVisible(items.length);
      return;
    }
    if (visible >= items.length) return;
    const timer = window.setTimeout(() => setVisible((value) => value + 1), delay);
    return () => window.clearTimeout(timer);
  }, [delay, items.length, reducedMotion, visible]);

  return (
    <div className={`flex flex-col gap-3 ${className ?? ""}`} {...props}>
      <AnimatePresence initial={false}>
        {items.slice(0, visible).map((item) => (
          <motion.div
            key={typeof item === "object" && item && "key" in item ? item.key : undefined}
            layout="position"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -8 }}
            transition={{ type: "spring", stiffness: 320, damping: 32 }}
          >
            {item}
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
});
