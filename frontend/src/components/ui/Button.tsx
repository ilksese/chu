import * as React from "react";
import { tv, type VariantProps } from "tailwind-variants";

const button = tv({
  base: "inline-flex cursor-pointer items-center justify-center gap-2 border-2 border-border font-bold transition disabled:cursor-not-allowed disabled:border-neutral-300 disabled:bg-neutral-200 disabled:text-neutral-400 disabled:shadow-none disabled:transform-none [&_svg]:shrink-0",
  variants: {
    variant: {
      primary:
        "bg-primary text-primary-foreground shadow-neo-button hover:bg-primary-hover hover:-translate-x-px hover:-translate-y-px hover:shadow-neo active:translate-x-0.5 active:translate-y-0.5 active:shadow-none",
      secondary:
        "bg-card text-foreground shadow-neo-sm hover:bg-muted active:translate-x-0.5 active:translate-y-0.5 active:shadow-none",
      danger: "bg-destructive text-destructive-foreground hover:bg-destructive-hover",
      info: "bg-info text-info-foreground hover:bg-info-hover",
      ghost:
        "border-0 bg-transparent text-muted-foreground shadow-none hover:bg-primary hover:text-foreground",
    },
    size: {
      md: "min-h-10 rounded-lg px-4 text-[13px] [&_svg]:size-4",
      sm: "min-h-8 rounded px-2.5 text-[10px] [&_svg]:size-3.5",
      icon: "size-10 rounded-md p-0 shadow-neo-sm hover:-translate-x-px hover:-translate-y-px hover:bg-primary-hover hover:shadow-neo active:translate-x-px active:translate-y-px active:shadow-none [&_svg]:size-4",
    },
  },
  defaultVariants: { variant: "secondary", size: "md" },
});

function Button({
  className,
  variant,
  size,
  ref,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof button> & { ref?: React.Ref<HTMLButtonElement> }) {
  return (
    <button
      ref={ref}
      data-slot="button"
      className={button({ variant, size, className })}
      {...props}
    />
  );
}

export { Button, button as buttonVariants };
