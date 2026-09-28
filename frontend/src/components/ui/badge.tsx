import type { ComponentProps } from "react"
import { tv, type VariantProps } from "tailwind-variants"

const badge = tv({
  base: "inline-flex items-center rounded-full border border-border font-bold leading-none",
  variants: {
    tone: {
      yellow: "bg-primary text-primary-foreground",
      neutral: "bg-muted text-foreground",
      warning: "bg-warning-surface text-warning-foreground",
      info: "bg-info-surface text-info",
    },
    size: {
      md: "min-h-[22px] px-2 text-[10px]",
      sm: "min-h-[18px] px-1.5 text-[8px]",
    },
  },
  defaultVariants: { tone: "yellow", size: "md" },
})

function Badge({ className, tone, size, ...props }: ComponentProps<"span"> & VariantProps<typeof badge>) {
  return <span className={badge({ tone, size, className })} {...props} />
}

export { Badge, badge }
