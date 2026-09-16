import { Activity, Bot, Network, Settings, Sparkles, type LucideIcon } from "lucide-react";

export type View = "overview" | "skills" | "mcps" | "agents" | "settings";

export type NavigationItem = {
  id: View;
  label: string;
  path: string;
  icon: LucideIcon;
};

export const navigation: NavigationItem[] = [
  { id: "overview", label: "总览", path: "/", icon: Activity },
  { id: "skills", label: "Skills", path: "/skills", icon: Sparkles },
  { id: "mcps", label: "MCP 服务", path: "/mcps", icon: Network },
  { id: "agents", label: "自定义 Agent", path: "/agents", icon: Bot },
  { id: "settings", label: "设置", path: "/settings", icon: Settings },
];

export const viewPaths = Object.fromEntries(
  navigation.map((item) => [item.id, item.path]),
) as Record<View, string>;

export function viewFromPath(pathname: string): View {
  const path = pathname.replace(/\/+$/, "") || "/";
  return navigation.find((item) => item.path === path)?.id ?? "overview";
}
