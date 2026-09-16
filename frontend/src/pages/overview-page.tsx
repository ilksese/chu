import { startTransition, useRef, useState, type CSSProperties } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Bot,
  Check,
  CircleAlert,
  FolderCog,
  Link2,
  MoveHorizontal,
  Network,
  Plus,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
  type LucideIcon,
} from "lucide-react";
import { useNavigate } from "react-router";
import { viewPaths } from "@/app/navigation";
import { AddResourceDialog } from "@/components/add-resource-dialog";
import { HostMark, StatusDot } from "@/components/host-controls";
import { resourceKinds, type ResourceKind } from "@/lib/resources";
import { useAppStore } from "@/stores/app-store";

type ShowcaseItem = {
  id: ResourceKind;
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
  onOpen: (kind: ResourceKind) => void;
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
      className="showcase"
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
      <div className="showcase-heading">
        <div>
          <h2>能力橱窗</h2>
          <p>横向拖动，浏览并进入你的 Agent 资源。</p>
        </div>
        <div className="carousel-actions">
          <button
            type="button"
            className="icon-button"
            aria-label="上一个功能"
            onClick={() => move(-1)}
          >
            <ArrowLeft />
          </button>
          <button
            type="button"
            className="icon-button"
            aria-label="下一个功能"
            onClick={() => move(1)}
          >
            <ArrowRight />
          </button>
        </div>
      </div>

      <div
        className="carousel-stage"
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
        <div className="carousel-orbit" aria-hidden="true" />
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
            <article key={item.id} className="showcase-card" data-active={active} style={style}>
              <button
                type="button"
                className="showcase-hit"
                aria-current={active ? "true" : undefined}
                aria-label={active ? `打开 ${item.label}` : `浏览 ${item.label}`}
                onClick={() => {
                  if (didDrag.current) return;
                  if (active) onOpen(item.id);
                  else onActiveChange(index);
                }}
              />
              <span className="showcase-card-topline">
                <span>{item.id.toUpperCase()}</span>
                <small>{String(index + 1).padStart(2, "0")}</small>
              </span>
              <span className="showcase-icon">
                <Icon />
              </span>
              <span className="showcase-copy">
                <strong>{item.label}</strong>
                <span>{item.description}</span>
              </span>
              <span className="showcase-card-footer">
                <span>
                  <strong>{item.count}</strong>
                  <small>{item.detail}</small>
                </span>
                <span className="showcase-open">
                  <ArrowRight />
                </span>
              </span>
            </article>
          );
        })}
      </div>

      <div className="carousel-footer">
        <span className="drag-hint">
          <MoveHorizontal />
          拖动或使用方向键
        </span>
        <div className="carousel-dots" aria-label="选择功能">
          {items.map((item, index) => (
            <button
              type="button"
              key={item.id}
              aria-label={`浏览 ${item.label}`}
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
  const [dialog, setDialog] = useState<ResourceKind>();
  const dialogTrigger = useRef<HTMLButtonElement>(null);
  const activeKind = resourceKinds[carouselIndex];
  const installedHosts = snapshot.hosts.filter((host) => host.installed).length;
  const activeDeployments = [...snapshot.skills, ...snapshot.mcps, ...snapshot.agents].reduce(
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
  ];

  function openView(kind: ResourceKind | "settings") {
    startTransition(() => navigate(viewPaths[kind]));
  }

  return (
    <>
      <section className="page-heading home-heading">
        <div>
          <h1>Agent 控制台</h1>
          <p>统一维护资源，按宿主独立交付。</p>
        </div>
        <button
          type="button"
          className="button button-primary"
          onClick={(event) => {
            dialogTrigger.current = event.currentTarget;
            setDialog(activeKind);
          }}
        >
          <Plus />
          添加资源
        </button>
      </section>

      <FeatureCarousel
        items={showcaseItems}
        activeIndex={carouselIndex}
        onActiveChange={setCarouselIndex}
        onOpen={openView}
      />

      <section className="stats-grid" aria-label="资源状态">
        <article>
          <span className="stat-icon stat-green">
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
        <article>
          <span className="stat-icon stat-yellow">
            <Link2 />
          </span>
          <div>
            <small>活动部署</small>
            <strong>{activeDeployments}</strong>
            <em>link 优先</em>
          </div>
        </article>
        <article>
          <span className="stat-icon stat-blue">
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
        <article>
          <span className="stat-icon stat-amber">
            <CircleAlert />
          </span>
          <div>
            <small>等待处理</small>
            <strong>{unmanaged}</strong>
            <em>{unmanaged ? "发现外部资源" : "没有配置冲突"}</em>
          </div>
        </article>
      </section>

      <section className="overview-lower">
        <div className="host-band">
          <div className="section-heading">
            <h2>Agent 宿主</h2>
            <button type="button" className="text-button" onClick={() => openView("settings")}>
              查看路径 <ArrowRight />
            </button>
          </div>
          <div className="host-grid">
            {snapshot.hosts.map((host) => (
              <article key={host.id} className="host-card">
                <HostMark host={host} />
                <div>
                  <strong>{host.name}</strong>
                  <span>
                    <StatusDot ready={host.installed} />
                    {host.installed ? "已连接" : "未检测到"}
                  </span>
                </div>
                <code>{host.format.toUpperCase()}</code>
              </article>
            ))}
          </div>
        </div>
        <aside className="insight-panel">
          <span className="insight-icon">
            <ShieldCheck />
          </span>
          <div>
            <h3>所有写入都有退路</h3>
            <p>宿主配置写入前保留上一份备份。检测到外部改动时，Chu 会停止覆盖。</p>
          </div>
          <div className="guard-list">
            <span>
              <Check />
              同名冲突阻断
            </span>
            <span>
              <Check />
              copy 改动保护
            </span>
            <span>
              <Check />
              敏感值默认掩码
            </span>
          </div>
        </aside>
      </section>

      {dialog ? (
        <AddResourceDialog
          kind={dialog}
          onClose={() => setDialog(undefined)}
          returnFocus={dialogTrigger.current}
        />
      ) : null}
    </>
  );
}
