import { useEffect } from "react";

export function useBodyScrollLock(locked = true) {
  useEffect(() => {
    if (!locked) return;
    const rootOverflow = document.documentElement.style.overflow;
    const bodyOverflow = document.body.style.overflow;
    document.documentElement.style.overflow = "hidden";
    document.body.style.overflow = "hidden";
    return () => {
      document.documentElement.style.overflow = rootOverflow;
      document.body.style.overflow = bodyOverflow;
    };
  }, [locked]);
}
