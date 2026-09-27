import { useLayoutEffect, useId, useRef, type CSSProperties, type ReactNode, type Ref, type ToggleEvent } from "react"
import { tv } from "tailwind-variants"

const popover = tv({
  base: "popover-anchor fixed z-80 m-0 w-[min(240px,calc(100vw-24px))] rounded-lg border-2 border-black bg-white p-3 text-black shadow-[4px_4px_0_#000]",
})

function Popover({
  open,
  anchor,
  onClose,
  className,
  children,
}: {
  open: boolean
  anchor: Ref<HTMLElement>
  onClose: () => void
  className?: string
  children: ReactNode
}) {
  const ref = useRef<HTMLDivElement>(null)
  const anchorName = `--popover-${useId().replace(/:/g, "")}`

  useLayoutEffect(() => {
    const target = anchor && typeof anchor !== "function" ? anchor.current : null
    const pop = ref.current
    if (!open || !target || !pop) return
    target.style.setProperty("anchor-name", anchorName)
    pop.style.setProperty("position-anchor", anchorName)
    if (!pop.matches(":popover-open")) pop.showPopover()
    return () => {
      target.style.removeProperty("anchor-name")
      if (pop.matches(":popover-open")) pop.hidePopover()
    }
  }, [anchor, anchorName, open])

  return (
    <div
      ref={ref}
      popover="auto"
      className={popover({ className })}
      style={{ visibility: open ? "visible" : "hidden" } as CSSProperties}
      onToggle={(event: ToggleEvent<HTMLDivElement>) => {
        if (event.newState === "closed" && open) onClose()
      }}
    >
      {children}
    </div>
  )
}

export { Popover }
