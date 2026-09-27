import * as React from "react"
import { tv, type VariantProps } from "tailwind-variants"

const input = tv({
  base: "min-w-0 border-2 border-black bg-white text-black outline-none focus:border-[#e92929] focus:shadow-[0_0_0_3px_rgb(233_41_41/15%)]",
  variants: {
    size: {
      md: "h-10 rounded-md px-3 text-xs",
      bare: "h-[34px] border-0 bg-transparent px-0 focus:shadow-none",
    },
  },
  defaultVariants: { size: "md" },
})

function Input({ className, size, type, ...props }: Omit<React.ComponentProps<"input">, "size"> & VariantProps<typeof input>) {
  return <input type={type} data-slot="input" className={input({ size, className })} {...props} />
}

export { Input, input }
