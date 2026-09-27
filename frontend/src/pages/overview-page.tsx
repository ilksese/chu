import { startTransition, useRef, useState, type CSSProperties } from "react";
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
  activeIndex,
  onActiveChange,
  onOpen,
}: {
  items: ShowcaseItem[];
  activeIndex: number;
  onActiveChange: (index: number) => void;
  onOpen: (kind: ShowcaseKind) => void;
}) {
  const dragStart = useRef<number | undefined>(undefined);
  const didDrag = useRef(false);

  function move(direction: -1 | 1) {
    onActiveChange((activeIndex + direction + items.length) % items.length);
  }

  function relativeOffset(index: number) {
    let offset = index - activeIndex;
    if (offset > items.length / 2) offset -= items.length;
    if (offset < -items.length / 2) offset += items.length;
    return offset;
  }

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
        className="relative mx-[-24px] mt-1 h-[360px] cursor-grab touch-pan-y overflow-hidden perspective-[1200px] select-none active:cursor-grabbing"
        onPointerDown={(event) => {
          dragStart.current = event.clientX;
          didDrag.current = false;
        }}
        onPointerUp={(event) => {
          if (dragStart.current !== undefined) {
            const distance = event.clientX - dragStart.current;
            didDrag.current = Math.abs(distance) > 44;
            if (didDrag.current) move(distance < 0 ? 1 : -1);
          }
          dragStart.current = undefined;
          window.setTimeout(() => {
            didDrag.current = false;
          }, 0);
        }}
        onPointerCancel={() => {
          dragStart.current = undefined;
          didDrag.current = false;
        }}
      >
        <div className="absolute bottom-[23px] left-1/2 h-[86px] w-[min(760px,82%)] -translate-x-1/2 rotate-x-[69deg] rounded-full border-2 border-dashed border-[#5c3613]/55" aria-hidden="true" />
        {items.map((item, index) => {
          const offset = relativeOffset(index);
          const Icon = item.icon;
          const active = offset === 0;
          const style = {
            "--carousel-x": `${offset * 76}%`,
            "--carousel-rotate": `${offset * -16}deg`,
            "--carousel-scale": active ? 1 : 0.82,
            "--carousel-depth": active ? "0px" : "-120px",
            "--carousel-opacity": active ? 1 : 0.78,
          } as CSSProperties;

          return (
            <article key={item.id} className="absolute top-1/2 left-1/2 z-1 grid h-[292px] w-[clamp(300px,36vw,430px)] grid-rows-[auto_1fr_auto] overflow-hidden rounded-lg border-2 border-black bg-white p-[18px] text-left text-black opacity-(--carousel-opacity) shadow-[2px_2px_0_#000] transition data-[active=true]:z-3 data-[active=true]:shadow-[6px_6px_0_#000]" data-active={active} style={{ ...style, transform: "translate(-50%, -50%) translateX(var(--carousel-x)) translateZ(var(--carousel-depth)) rotateY(var(--carousel-rotate)) scale(var(--carousel-scale))" }}>
              <button
                type="button"
                className="absolute inset-0 z-4 cursor-pointer rounded-md border-0 bg-transparent p-0"
                aria-current={active ? "true" : undefined}
                aria-label={active ? `打开 ${item.label}` : `浏览 ${item.label}`}
                onClick={() => {
                  if (didDrag.current) return;
                  if (active) onOpen(item.id);
                  else onActiveChange(index);
                }}
              />
              <span className="flex items-center justify-between text-[10px] font-extrabold text-[#5c3613]">
                <span>{item.id.toUpperCase()}</span>
                <small>{String(index + 1).padStart(2, "0")}</small>
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
          );
        })}
      </div>

      <div className="flex min-h-7 items-center justify-between text-[#5c3613]">
        <span className="flex items-center gap-1.5 text-[11px] [&_svg]:size-4">
          <MoveHorizontal />
          拖动或使用方向键
        </span>
        <div className="flex gap-1.5" aria-label="选择功能">
          {items.map((item, index) => (
            <button
              type="button"
              key={item.id}
              aria-label={`浏览 ${item.label}`}
              className="h-2 w-[26px] cursor-pointer rounded-full border border-black bg-[#cccccc] p-0 aria-[current=true]:scale-x-145 aria-[current=true]:bg-primary"
              aria-current={index === activeIndex ? "true" : undefined}
              onClick={() => onActiveChange(index)}
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
  const [carouselIndex, setCarouselIndex] = useState(0);
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
        activeIndex={carouselIndex}
        onActiveChange={setCarouselIndex}
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
