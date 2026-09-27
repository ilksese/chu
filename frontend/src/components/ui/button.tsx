import * as React from "react"
import { tv, type VariantProps } from "tailwind-variants"

const button = tv({
  base: "inline-flex cursor-pointer items-center justify-center gap-2 border-2 border-black font-bold transition disabled:cursor-not-allowed disabled:border-neutral-300 disabled:bg-neutral-200 disabled:text-neutral-400 disabled:shadow-none disabled:transform-none [&_svg]:shrink-0",
  variants: {
    variant: {
      primary: "bg-primary text-primary-foreground shadow-[3px_3px_0_#000] hover:bg-[#ffe62d] hover:-translate-x-px hover:-translate-y-px hover:shadow-[4px_4px_0_#000] active:translate-x-0.5 active:translate-y-0.5 active:shadow-none",
      secondary: "bg-white text-black shadow-[2px_2px_0_#000] hover:bg-[#f7f5ec] active:translate-x-0.5 active:translate-y-0.5 active:shadow-none",
      danger: "bg-[#e92929] text-white hover:bg-[#c41f1f]",
      info: "bg-[#2469d8] text-white hover:bg-[#1d56b3]",
      ghost: "border-0 bg-transparent text-[#5c3613] shadow-none hover:bg-primary hover:text-black",
    },
    size: {
      md: "min-h-10 rounded-lg px-4 text-[13px] [&_svg]:size-4",
      sm: "min-h-8 rounded px-2.5 text-[10px] [&_svg]:size-3.5",
      icon: "size-10 rounded-md p-0 shadow-[2px_2px_0_#000] hover:-translate-x-px hover:-translate-y-px hover:bg-[#ffe62d] hover:shadow-[4px_4px_0_#000] active:translate-x-px active:translate-y-px active:shadow-none [&_svg]:size-4",
    },
  },
  defaultVariants: { variant: "secondary", size: "md" },
})

function Button({
  className,
  variant,
  size,
  ref,
  ...props
}: React.ComponentProps<"button"> & VariantProps<typeof button> & { ref?: React.Ref<HTMLButtonElement> }) {
  return <button ref={ref} data-slot="button" className={button({ variant, size, className })} {...props} />
}

export { Button, button as buttonVariants }
