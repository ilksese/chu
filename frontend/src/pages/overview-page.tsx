import { startTransition, useEffect, useRef, useState } from "react";
import {
  animate,
  motion,
  useMotionValue,
  useMotionValueEvent,
  useReducedMotion,
} from "motion/react";
import { tv } from "tailwind-variants";
import {
  ArrowLeft,
  ArrowRight,
  Bot,
  CircleAlert,
  FolderCog,
  Link2,
  MoveHorizontal,
  Network,
  ScrollText,
  Sparkles,
  TerminalSquare,
  type LucideIcon,
} from "lucide-react";
import { useNavigate } from "react-router";
import { viewPaths, type View } from "@/app/navigation";
import { HostMark, StatusDot } from "@/components/host-controls";
import { Button } from "@/components/ui/button";
import type { ResourceKind } from "@/lib/resources";
import { useAppStore } from "@/stores/app-store";

type ShowcaseKind = ResourceKind | "prompts";

const iconButton = "inline-grid size-10 cursor-pointer place-items-center rounded-md border-2 border-black bg-white shadow-[2px_2px_0_#000] hover:-translate-x-px hover:-translate-y-px hover:bg-[#ffe62d] [&_svg]:size-4";
const statIcon = tv({
  base: "grid size-[38px] shrink-0 place-items-center rounded-md border border-black [&_svg]:size-[18px]",
  variants: { tone: { green: "bg-[#e8f8ec] text-[#229948]", yellow: "bg-primary text-black", blue: "bg-[#e8f0fc] text-[#2469d8]", amber: "bg-[#fde8c8] text-[#5c3613]" } },
});

type ShowcaseItem = {
  id: ShowcaseKind;
  label: string;
  description: string;
  detail: string;
  count: number;
  icon: LucideIcon;
};

function FeatureCarousel({
  items,
  onOpen,
}: {
  items: ShowcaseItem[];
  onOpen: (kind: ShowcaseKind) => void;
}) {
  const ringCopies = 3;
  const slots = Array.from({ length: ringCopies }, (_, copyIndex) =>
    items.map((item, itemIndex) => ({
      item,
      itemIndex,
      slotIndex: copyIndex * items.length + itemIndex,
    })),
  ).flat();
  const angleStep = 360 / slots.length;
  const ringRadius = 820;
  const rotation = useMotionValue(0);
  const prefersReducedMotion = useReducedMotion();
  const animation = useRef<{ stop: () => void } | null>(null);
  const targetRotation = useRef(0);
  const drag = useRef<{
    pointerId: number;
    startX: number;
    startRotation: number;
    lastX: number;
    lastTime: number;
    velocity: number;
  } | null>(null);
  const wheel = useRef<{ lastTime: number; velocity: number; timeout?: number }>({
    lastTime: 0,
    velocity: 0,
  });
  const didDrag = useRef(false);
  const [frontSlot, setFrontSlot] = useState(0);

  function modulo(value: number, divisor: number) {
    return ((value % divisor) + divisor) % divisor;
  }

  useMotionValueEvent(rotation, "change", (latest) => {
    const slot = Math.round(-latest / angleStep);
    setFrontSlot((current) => (current === slot ? current : slot));
  });

  useEffect(
    () => () => {
      animation.current?.stop();
      if (wheel.current.timeout) window.clearTimeout(wheel.current.timeout);
    },
    [],
  );

  function animateToSlot(slot: number) {
    const target = -slot * angleStep;
    targetRotation.current = target;
    animation.current?.stop();
    if (prefersReducedMotion) {
      rotation.set(target);
      return;
    }
    animation.current = animate(rotation, target, {
      type: "tween",
      duration: 0.34,
      ease: [0.16, 1, 0.3, 1],
    });
  }

  function coast(velocity: number) {
    animation.current?.stop();
    const current = rotation.get();
    const target = prefersReducedMotion
      ? Math.round(current / angleStep) * angleStep
      : Math.round((current + velocity * 0.22) / angleStep) * angleStep;
    targetRotation.current = target;

    if (prefersReducedMotion) {
      rotation.set(target);
      return;
    }

    animation.current = animate(rotation, 0, {
      type: "inertia",
      velocity,
      power: 0.22,
      timeConstant: 260,
      restDelta: 0.1,
      modifyTarget: () => target,
    });
  }

  function move(direction: -1 | 1) {
    const currentSlot = Math.round(-targetRotation.current / angleStep);
    animateToSlot(currentSlot + direction);
  }

  function selectItem(itemIndex: number) {
    const currentSlot = Math.round(-targetRotation.current / angleStep);
    let distance = itemIndex - modulo(currentSlot, items.length);
    if (distance > items.length / 2) distance -= items.length;
    if (distance < -items.length / 2) distance += items.length;
    animateToSlot(currentSlot + distance);
  }

  function selectRenderedSlot(slotIndex: number) {
    const currentSlot = Math.round(-targetRotation.current / angleStep);
    const nearestSlot =
      slotIndex + Math.round((currentSlot - slotIndex) / slots.length) * slots.length;
    animateToSlot(nearestSlot);
  }

  function finishDrag(cancelled = false) {
    const elapsed = drag.current ? performance.now() - drag.current.lastTime : Infinity;
    const velocity = !cancelled && elapsed < 100 ? (drag.current?.velocity ?? 0) : 0;
    drag.current = null;
    coast(velocity);
    window.setTimeout(() => {
      didDrag.current = false;
    }, 0);
  }

  const activeIndex = modulo(frontSlot, items.length);
  const activeRenderedSlot = modulo(frontSlot, slots.length);

  return (
    <section
      className="overflow-hidden rounded-lg border-2 border-black bg-[#f7f5ec] p-6 shadow-[4px_4px_0_#000]"
      aria-label="功能浏览"
      aria-roledescription="carousel"
      tabIndex={0}
      onKeyDown={(event) => {
        if (event.key === "ArrowLeft") {
          event.preventDefault();
          move(-1);
        }
        if (event.key === "ArrowRight") {
          event.preventDefault();
          move(1);
        }
      }}
    >
      <div>
        <div className="flex gap-2.5">
          <button
            type="button"
            className={iconButton}
            aria-label="上一个功能"
            onClick={() => move(-1)}
          >
            <ArrowLeft />
          </button>
          <button
            type="button"
            className={iconButton}
            aria-label="下一个功能"
            onClick={() => move(1)}
          >
            <ArrowRight />
          </button>
        </div>
      </div>

      <div
        className="relative mx-[-24px] mt-1 h-[360px] cursor-grab touch-pan-y overflow-hidden perspective-[1100px] select-none active:cursor-grabbing"
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          animation.current?.stop();
          event.currentTarget.setPointerCapture(event.pointerId);
          drag.current = {
            pointerId: event.pointerId,
            startX: event.clientX,
            startRotation: rotation.get(),
            lastX: event.clientX,
            lastTime: performance.now(),
            velocity: 0,
          };
          didDrag.current = false;
        }}
        onPointerMove={(event) => {
          if (drag.current?.pointerId !== event.pointerId) return;
          const distance = event.clientX - drag.current.startX;
          if (Math.abs(distance) > 5 && !didDrag.current) {
            didDrag.current = true;
            event.currentTarget.parentElement?.focus();
          }
          const now = performance.now();
          const elapsed = now - drag.current.lastTime;
          if (elapsed > 0) {
            const velocity = ((event.clientX - drag.current.lastX) * 0.16 * 1000) / elapsed;
            drag.current.velocity = drag.current.velocity * 0.55 + velocity * 0.45;
            drag.current.lastX = event.clientX;
            drag.current.lastTime = now;
          }
          const nextRotation = drag.current.startRotation + distance * 0.16;
          rotation.set(nextRotation);
        }}
        onPointerUp={(event) => {
          if (drag.current?.pointerId !== event.pointerId) return;
          event.currentTarget.releasePointerCapture(event.pointerId);
          finishDrag();
        }}
        onPointerCancel={(event) => {
          if (drag.current?.pointerId !== event.pointerId) return;
          finishDrag(true);
        }}
        onWheel={(event) => {
          const distance =
            Math.abs(event.deltaX) > Math.abs(event.deltaY)
              ? event.deltaX
              : event.shiftKey
                ? event.deltaY
                : 0;
          if (!distance) return;
          const now = performance.now();
          const elapsed = wheel.current.lastTime ? now - wheel.current.lastTime : 16;
          const delta = -distance * 0.12;
          const velocity = (delta * 1000) / Math.max(elapsed, 1);
          wheel.current.velocity = wheel.current.velocity * 0.45 + velocity * 0.55;
          wheel.current.lastTime = now;
          animation.current?.stop();
          rotation.set(rotation.get() + delta);
          if (wheel.current.timeout) window.clearTimeout(wheel.current.timeout);
          wheel.current.timeout = window.setTimeout(() => {
            coast(wheel.current.velocity);
            wheel.current.lastTime = 0;
            wheel.current.velocity = 0;
            wheel.current.timeout = undefined;
          }, 90);
        }}
      >
        <div
          className="absolute bottom-[23px] left-1/2 h-[86px] w-[min(760px,82%)] -translate-x-1/2 rotate-x-[69deg] rounded-full border-2 border-dashed border-[#5c3613]/55"
          aria-hidden="true"
        />
        <div
          className="absolute inset-0 [transform-style:preserve-3d]"
          style={{ transform: `translateZ(-${ringRadius}px)` }}
        >
          <motion.div
            className="absolute inset-0 [transform-style:preserve-3d]"
            style={{ rotateY: rotation }}
          >
            {slots.map(({ item, itemIndex, slotIndex }) => {
              const Icon = item.icon;
              const active = activeRenderedSlot === slotIndex;
              const nearestAccessibleSlot =
                itemIndex + Math.round((frontSlot - itemIndex) / items.length) * items.length;
              const accessible = modulo(nearestAccessibleSlot, slots.length) === slotIndex;

              return (
                <div
                  key={`${item.id}-${slotIndex}`}
                  className="absolute top-1/2 left-1/2 size-0 [transform-style:preserve-3d]"
                  style={{
                    transform: `rotateY(${slotIndex * angleStep}deg) translateZ(${ringRadius}px)`,
                  }}
                >
                  <article
                    aria-hidden={accessible ? undefined : true}
                    className={`relative grid h-[292px] w-[clamp(300px,36vw,430px)] -translate-x-1/2 -translate-y-1/2 grid-rows-[auto_1fr_auto] overflow-hidden rounded-lg border-2 border-black bg-white p-[18px] text-left text-black shadow-[2px_2px_0_#000] transition-[box-shadow] duration-200 motion-reduce:transition-none data-[active=true]:shadow-[6px_6px_0_#000] ${accessible ? "" : "pointer-events-none"}`}
                    data-active={active}
                    style={{ backfaceVisibility: "hidden" }}
                  >
                    <button
                      type="button"
                      className="absolute inset-0 z-4 cursor-pointer rounded-md border-0 bg-transparent p-0"
                      aria-current={active ? "true" : undefined}
                      aria-label={active ? `打开 ${item.label}` : `浏览 ${item.label}`}
                      tabIndex={accessible ? 0 : -1}
                      onClick={() => {
                        if (didDrag.current) return;
                        if (active) onOpen(item.id);
                        else selectRenderedSlot(slotIndex);
                      }}
                    />
                    <span className="flex items-center justify-between text-[10px] font-extrabold text-[#5c3613]">
                      <span>{item.id.toUpperCase()}</span>
                      <small>{String(itemIndex + 1).padStart(2, "0")}</small>
                    </span>
                    <span className="absolute top-[50px] right-[18px] grid size-12 place-items-center rounded-md border-2 border-black bg-primary shadow-[2px_2px_0_#000] [&_svg]:size-6">
                      <Icon />
                    </span>
                    <span className="max-w-[72%] self-end pb-6 [&_strong]:mb-2 [&_strong]:block [&_strong]:text-[29px] [&_strong]:leading-none [&_span]:block [&_span]:text-[13px] [&_span]:leading-normal [&_span]:text-[#5c3613]">
                      <strong>{item.label}</strong>
                      <span>{item.description}</span>
                    </span>
                    <span className="flex items-center justify-between border-t border-[#5c3613] pt-3.5">
                      <span>
                        <strong>{item.count}</strong>
                        <small>{item.detail}</small>
                      </span>
                      <span className="grid size-9 place-items-center rounded-full border-2 border-black bg-primary [&_svg]:size-4">
                        <ArrowRight />
                      </span>
                    </span>
                  </article>
                </div>
              );
            })}
          </motion.div>
        </div>
      </div>

      <div className="flex min-h-7 items-center justify-between text-[#5c3613]">
        <span className="flex items-center gap-1.5 text-[11px] [&_svg]:size-4">
          <MoveHorizontal />
          拖动、横向滚轮或方向键
        </span>
        <div className="flex gap-1.5" aria-label="选择功能">
          {items.map((item, index) => (
            <button
              type="button"
              key={item.id}
              aria-label={`浏览 ${item.label}`}
              className="h-2 w-[26px] cursor-pointer rounded-full border border-black bg-[#cccccc] p-0 aria-[current=true]:scale-x-145 aria-[current=true]:bg-primary"
              aria-current={index === activeIndex ? "true" : undefined}
              onClick={() => selectItem(index)}
            />
          ))}
        </div>
        <strong>
          {String(activeIndex + 1).padStart(2, "0")} / {String(items.length).padStart(2, "0")}
        </strong>
      </div>
    </section>
  );
}

export function OverviewPage() {
  const snapshot = useAppStore((state) => state.snapshot);
  const navigate = useNavigate();
  const installedHosts = snapshot.hosts.filter((host) => host.installed).length;
  const activeDeployments = [...snapshot.skills, ...snapshot.mcps, ...snapshot.agents, ...(snapshot.prompts ?? [])].reduce(
    (total, item) => total + Object.values(item.enabledOn).filter(Boolean).length,
    0,
  );
  const unmanaged = [...snapshot.skills, ...snapshot.mcps, ...snapshot.agents].filter(
    (item) => !item.managed,
  ).length;
  const showcaseItems: ShowcaseItem[] = [
    {
      id: "skills",
      label: "Skills",
      description: "安装、导入并向宿主部署可复用能力。",
      detail: "个技能",
      count: snapshot.skills.length,
      icon: Sparkles,
    },
    {
      id: "mcps",
      label: "MCP 服务",
      description: "集中管理工具连接与本地服务配置。",
      detail: "个连接",
      count: snapshot.mcps.length,
      icon: Network,
    },
    {
      id: "agents",
      label: "自定义 Agent",
      description: "共享角色定义，并按宿主独立启用。",
      detail: "个角色",
      count: snapshot.agents.length,
      icon: Bot,
    },
    {
      id: "prompts",
      label: "提示词",
      description: "维护全局提示词，并替换到各个宿主。",
      detail: "份提示词",
      count: snapshot.prompts?.length ?? 0,
      icon: ScrollText,
    },
  ];

  function openView(kind: View) {
    startTransition(() => navigate(viewPaths[kind]));
  }

  return (
    <>
      <FeatureCarousel
        items={showcaseItems}
        onOpen={openView}
      />

      <section className="mt-6 grid grid-cols-4 overflow-hidden rounded-lg border-2 border-black bg-white max-[1120px]:grid-cols-2" aria-label="资源状态">
        <article className="flex min-w-0 items-center gap-3 border-r border-black p-4 last:border-r-0 [&_div]:grid [&_div]:min-w-0 [&_small]:truncate [&_small]:text-[10px] [&_small]:text-[#5c3613] [&_em]:truncate [&_em]:text-[10px] [&_em]:text-[#5c3613] [&_em]:not-italic [&_strong]:text-[23px] [&_strong]:tabular-nums [&_strong_span]:text-xs [&_strong_span]:text-[#5c3613]">
          <span className={statIcon({ tone: "green" })}>
            <TerminalSquare />
          </span>
          <div>
            <small>已接入宿主</small>
            <strong>
              {installedHosts}
              <span> / {snapshot.hosts.length}</span>
            </strong>
            <em>自动扫描本机</em>
          </div>
        </article>
        <article className="flex min-w-0 items-center gap-3 border-r border-black p-4 last:border-r-0 [&_div]:grid [&_div]:min-w-0 [&_small]:truncate [&_small]:text-[10px] [&_small]:text-[#5c3613] [&_em]:truncate [&_em]:text-[10px] [&_em]:text-[#5c3613] [&_em]:not-italic [&_strong]:text-[23px] [&_strong]:tabular-nums [&_strong_span]:text-xs [&_strong_span]:text-[#5c3613]">
          <span className={statIcon({ tone: "yellow" })}>
            <Link2 />
          </span>
          <div>
            <small>活动部署</small>
            <strong>{activeDeployments}</strong>
            <em>link 优先</em>
          </div>
        </article>
        <article className="flex min-w-0 items-center gap-3 border-r border-black p-4 last:border-r-0 [&_div]:grid [&_div]:min-w-0 [&_small]:truncate [&_small]:text-[10px] [&_small]:text-[#5c3613] [&_em]:truncate [&_em]:text-[10px] [&_em]:text-[#5c3613] [&_em]:not-italic [&_strong]:text-[23px] [&_strong]:tabular-nums [&_strong_span]:text-xs [&_strong_span]:text-[#5c3613]">
          <span className={statIcon({ tone: "blue" })}>
            <FolderCog />
          </span>
          <div>
            <small>中央资源</small>
            <strong>
              {snapshot.skills.filter((item) => item.managed).length +
                snapshot.mcps.filter((item) => item.managed).length +
                snapshot.agents.filter((item) => item.managed).length}
            </strong>
            <em>位于 {snapshot.root}</em>
          </div>
        </article>
        <article className="flex min-w-0 items-center gap-3 border-r border-black p-4 last:border-r-0 [&_div]:grid [&_div]:min-w-0 [&_small]:truncate [&_small]:text-[10px] [&_small]:text-[#5c3613] [&_em]:truncate [&_em]:text-[10px] [&_em]:text-[#5c3613] [&_em]:not-italic [&_strong]:text-[23px] [&_strong]:tabular-nums [&_strong_span]:text-xs [&_strong_span]:text-[#5c3613]">
          <span className={statIcon({ tone: "amber" })}>
            <CircleAlert />
          </span>
          <div>
            <small>等待处理</small>
            <strong>{unmanaged}</strong>
            <em>{unmanaged ? "发现外部资源" : "没有配置冲突"}</em>
          </div>
        </article>
      </section>

      <section className="mt-8 min-w-0">
        <div className="mb-3 flex justify-end">
          <Button type="button" variant="ghost" onClick={() => openView("settings")}>
            查看路径 <ArrowRight />
          </Button>
        </div>
        <div className="grid grid-cols-3 gap-3 max-[1120px]:grid-cols-1">
          {snapshot.hosts.map((host) => (
            <article key={host.id} className="grid min-w-0 grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 rounded-lg border-2 border-black bg-white p-3 shadow-[2px_2px_0_#000]">
              <HostMark host={host} />
              <div className="grid min-w-0">
                <strong className="truncate text-xs">{host.name}</strong>
                <span className="mt-1 flex items-center gap-1.5 text-[10px] text-[#5c3613]">
                  <StatusDot ready={host.installed} />
                  {host.installed ? "已连接" : "未检测到"}
                </span>
              </div>
              <code className="text-[9px] text-[#5c3613]">{host.format.toUpperCase()}</code>
            </article>
          ))}
        </div>
      </section>

    </>
  );
}
