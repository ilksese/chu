import type { ComponentProps } from "react"
import { tv } from "tailwind-variants"

const card = tv({ base: "rounded-lg border-2 border-black bg-white text-black shadow-[2px_2px_0_#000]" })
const cardHeader = tv({ base: "grid items-start gap-2 px-6" })
const cardTitle = tv({ base: "font-semibold leading-none" })
const cardDescription = tv({ base: "text-sm text-[#5c3613]" })
const cardAction = tv({ base: "col-start-2 row-span-2 row-start-1 self-start justify-self-end" })
const cardContent = tv({ base: "px-6" })
const cardFooter = tv({ base: "flex items-center px-6" })

function Card({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="card" className={card({ className })} {...props} />
}
function CardHeader({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="card-header" className={cardHeader({ className })} {...props} />
}
function CardTitle({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="card-title" className={cardTitle({ className })} {...props} />
}
function CardDescription({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="card-description" className={cardDescription({ className })} {...props} />
}
function CardAction({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="card-action" className={cardAction({ className })} {...props} />
}
function CardContent({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="card-content" className={cardContent({ className })} {...props} />
}
function CardFooter({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="card-footer" className={cardFooter({ className })} {...props} />
}

export { Card, CardHeader, CardFooter, CardTitle, CardAction, CardDescription, CardContent }
