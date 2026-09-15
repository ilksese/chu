import React, { useRef, type PropsWithChildren } from "react"
import { cva, type VariantProps } from "class-variance-authority"
import {
  motion,
  type MotionProps,
  type MotionValue,
  useMotionValue,
  useReducedMotion,
  useSpring,
  useTransform,
} from "motion/react"

import { cn } from "@/lib/utils"

const dockVariants = cva(
  "flex items-center justify-center gap-2 border-2 border-black bg-white p-2 shadow-[3px_3px_0_#000]"
)

type DockOrientation = "horizontal" | "vertical"

interface DockProps extends VariantProps<typeof dockVariants> {
  className?: string
  iconSize?: number
  iconMagnification?: number
  disableMagnification?: boolean
  iconDistance?: number
  direction?: "top" | "middle" | "bottom"
  orientation?: DockOrientation
  children: React.ReactNode
}

const Dock = React.forwardRef<HTMLDivElement, DockProps>(
  (
    {
      className,
      children,
      iconSize = 42,
      iconMagnification = 54,
      disableMagnification = false,
      iconDistance = 110,
      direction = "middle",
      orientation = "horizontal",
      ...props
    },
    ref
  ) => {
    const pointerPosition = useMotionValue({ x: Infinity, y: Infinity })
    const prefersReducedMotion = useReducedMotion()

    return (
      <motion.div
        ref={ref}
        onPointerMove={(event) => pointerPosition.set({ x: event.clientX, y: event.clientY })}
        onPointerLeave={() => pointerPosition.set({ x: Infinity, y: Infinity })}
        {...props}
        className={cn(
          dockVariants({ className }),
          orientation === "vertical" && "flex-col",
          direction === "top" && "items-start",
          direction === "middle" && "items-center",
          direction === "bottom" && "items-end"
        )}
      >
        {React.Children.map(children, (child) =>
          React.isValidElement<DockIconProps>(child) && child.type === DockIcon
            ? React.cloneElement(child, {
                ...child.props,
                pointerPosition,
                size: iconSize,
                magnification: iconMagnification,
                disableMagnification: disableMagnification || Boolean(prefersReducedMotion),
                distance: iconDistance,
              })
            : child
        )}
      </motion.div>
    )
  }
)

Dock.displayName = "Dock"

interface DockIconProps extends Omit<MotionProps & React.HTMLAttributes<HTMLDivElement>, "children"> {
  size?: number
  magnification?: number
  disableMagnification?: boolean
  distance?: number
  pointerPosition?: MotionValue<{ x: number; y: number }>
  className?: string
  children?: React.ReactNode
  props?: PropsWithChildren
}

const DockIcon = ({
  size = 42,
  magnification = 54,
  disableMagnification,
  distance = 110,
  pointerPosition,
  className,
  children,
  ...props
}: DockIconProps) => {
  const ref = useRef<HTMLDivElement>(null)
  const fallbackPosition = useMotionValue({ x: Infinity, y: Infinity })
  const distanceFromPointer = useTransform(pointerPosition ?? fallbackPosition, (value) => {
    const bounds = ref.current?.getBoundingClientRect() ?? { x: 0, y: 0, width: 0, height: 0 }
    const dx = value.x - (bounds.x + bounds.width / 2)
    const dy = value.y - (bounds.y + bounds.height / 2)
    return Math.hypot(dx, dy)
  })
  const targetSize = disableMagnification ? size : magnification
  const transformedSize = useTransform(distanceFromPointer, [0, distance], [targetSize, size])
  const springSize = useSpring(transformedSize, { mass: 0.1, stiffness: 180, damping: 16 })

  return (
    <motion.div
      ref={ref}
      style={{ width: springSize, height: springSize }}
      className={cn("flex aspect-square shrink-0 items-center justify-center", className)}
      {...props}
    >
      {children}
    </motion.div>
  )
}

DockIcon.displayName = "DockIcon"

export { Dock, DockIcon }
