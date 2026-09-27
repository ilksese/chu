import { useEffect, useId, useRef, type CSSProperties, type ReactNode, type Ref, type ToggleEvent } from "react"
import { tv } from "tailwind-variants"

const popover = tv({
  base: "fixed inset-auto m-0 w-[min(240px,calc(100vw-24px))] overflow-visible rounded-lg border-2 border-black bg-white p-3 text-black shadow-[4px_4px_0_#000] before:absolute before:inset-[-8px] before:-z-1 position-area-[block-start_span-inline-end] position-try-fallbacks-[flip-block,flip-inline,flip-block_flip-inline]",
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

  useEffect(() => {
    const target = anchor && typeof anchor !== "function" ? anchor.current : null
    if (!target) return
    target.style.setProperty("anchor-name", anchorName)
    return () => {
      target.style.removeProperty("anchor-name")
    }
  }, [anchor, anchorName])

  useEffect(() => {
    if (!open) return
    ref.current?.showPopover()
  }, [open])

  if (!open) return null
  return (
    <div
      ref={ref}
      popover="auto"
      className={popover({ className })}
      style={{ positionAnchor: anchorName } as CSSProperties}
      onToggle={(event: ToggleEvent<HTMLDivElement>) => {
        if (event.newState === "closed") onClose()
      }}
    >
      {children}
    </div>
  )
}

export { Popover }
