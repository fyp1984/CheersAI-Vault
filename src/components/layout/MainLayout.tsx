import { Outlet, useLocation } from "react-router-dom";
import { useEffect, useState } from "react";
import { Wifi, WifiOff, Globe, RefreshCw } from "lucide-react";
import { Sidebar } from "./Sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { isTauriHost } from "@/lib/runtime/host";
import { fetchRuntimeFileBayStatus } from "@/lib/runtime/client";
import { useRuntimeHealthStore } from "@/store/runtimeStore";
import type { RuntimeFileBayConfigStatus } from "@/types/runtime";

const pageMeta: Record<string, { title: string; description: string }> = {
  "/cloud": {
    title: "CheersAI 云端工作区",
    description: "把敏感数据留在本地，让AI能力触手可及。",
  },
  "/process": {
    title: "文件脱敏",
    description: "本地离线处理与脱敏执行",
  },
  "/unmask": {
    title: "文件反脱敏",
    description: "安全还原已脱敏文件",
  },
  "/files": {
    title: "文件管理",
    description: "统一管理本地处理文件",
  },
  "/gitea": {
    title: "FileBay 设置",
    description: "连接与配置 FileBay",
  },
  "/enhanced": {
    title: "增强服务",
    description: "安装和管理 OCR 等增强功能",
  },
  "/rules": {
    title: "规则配置",
    description: "管理脱敏规则与策略",
  },
  "/sandbox": {
    title: "沙箱管理",
    description: "本地安全目录与 PIN 管理",
  },
  "/log": {
    title: "操作日志",
    description: "查看本地审计与操作记录",
  },
};

type FileBayConfigState =
  | { kind: "checking" }
  | { kind: "ready"; status: RuntimeFileBayConfigStatus }
  | { kind: "unreachable" };

interface StatusPillProps {
  label: string;
  detail: string;
  tone: string;
  Icon: typeof Globe;
  dot: string;
  panel: string;
  ariaLabel?: string;
  onRetry?: () => void;
}

function StatusPill({ label, detail, tone, Icon, dot, panel, ariaLabel, onRetry }: StatusPillProps) {
  return (
    <div
      className={`flex shrink-0 items-center gap-3 rounded-2xl border px-4 py-2 ${panel}`}
      aria-label={ariaLabel ?? label}
    >
      <div className={`h-2.5 w-2.5 rounded-full ${dot}`} />
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <Icon className={`h-4 w-4 ${tone}`} />
          <span className={`text-sm font-semibold ${tone}`}>{label}</span>
          {onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="inline-flex h-6 shrink-0 items-center gap-1 rounded-md border border-slate-200 bg-white px-1.5 text-[11px] font-medium text-slate-600 hover:text-slate-900"
            >
              <RefreshCw className="h-3 w-3" />
              重试
            </button>
          )}
        </div>
        <div className="truncate text-xs text-slate-500">{detail}</div>
      </div>
    </div>
  );
}

export function MainLayout() {
  const location = useLocation();
  const isDesktopHost = isTauriHost();
  const isDesktopWorkspace = location.pathname.startsWith("/cloud");
  const isGiteaSettings = location.pathname.startsWith("/gitea");
  const meta = pageMeta[location.pathname] ?? {
    title: isDesktopHost ? "CheersAI Vault" : "CheersAI Vault Pro",
    description: "把敏感数据留在本地，让AI能力触手可及。",
  };
  // 浏览器宿主页头不得出现 "CheersAI Desktop"（该名称仅指智能体工作台，
  // 不得用于本项目浏览器版）；桌面宿主 `/cloud` 内嵌的正是智能体工作台
  // 子 WebView，保留其原名属已确认的指向性用法。
  const headerTitle = isDesktopWorkspace
    ? (isDesktopHost ? "CheersAI Desktop 在线工作区" : "CheersAI 云端工作区")
    : meta.title;

  const runtimeStatus = useRuntimeHealthStore((state) => state.status);
  const refreshRuntime = useRuntimeHealthStore((state) => state.refresh);
  const [filebayConfig, setFilebayConfig] = useState<FileBayConfigState>({ kind: "checking" });

  // 浏览器宿主的 FileBay 配置状态只在 `/gitea` 页面读取（仅使用 Runtime 返回的
  // `status` 安全字段，不读取/展示/修改 Token、URL、owner、repo）；该只读接口
  // 从不触发 FileBay 出站请求。桌面宿主继续使用原有语义，不在此处读取。
  // 把 `runtimeStatus` 纳入依赖：Runtime 断开→恢复后自动重新读取配置状态，
  // 避免保留“状态不可用”的旧文案。
  useEffect(() => {
    if (isDesktopHost || !isGiteaSettings) {
      setFilebayConfig({ kind: "checking" });
      return;
    }
    let active = true;
    setFilebayConfig({ kind: "checking" });
    void fetchRuntimeFileBayStatus().then((result) => {
      if (!active) {
        return;
      }
      if (!result.ok) {
        setFilebayConfig({ kind: "unreachable" });
        return;
      }
      setFilebayConfig({ kind: "ready", status: result.data.status });
    });
    return () => {
      active = false;
    };
  }, [isDesktopHost, isGiteaSettings, runtimeStatus]);

  // 桌面宿主：保持既有"在线/离线"路由语义与"当前操作 …"文案不变。
  if (isDesktopHost) {
    const desktopStatus = (isDesktopWorkspace || isGiteaSettings)
      ? {
          label: "在线",
          detail: isDesktopWorkspace
            ? "当前操作 CheersAI Desktop 在线工作区"
            : "当前操作 FileBay 在线服务",
          tone: "text-emerald-600",
          Icon: Globe,
          dot: "bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]",
          panel: "border-emerald-100 bg-emerald-50/80",
        }
      : {
          label: "离线",
          detail: "当前操作 Vault 本地工作区",
          tone: "text-slate-600",
          Icon: WifiOff,
          dot: "bg-slate-400 shadow-[0_0_0_4px_rgba(148,163,184,0.12)]",
          panel: "border-slate-100 bg-slate-50/80",
        };

    return (
      <TooltipProvider>
        <div className="flex h-screen w-screen overflow-hidden bg-gray-50">
          <Sidebar />
          <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-white">
            <header className="flex h-[120px] shrink-0 items-center justify-between border-b border-slate-200 bg-white px-6">
              <div className="min-w-0 max-w-[760px] pr-6">
                <div className="text-sm font-semibold leading-6 text-slate-900">{headerTitle}</div>
                <div className="mt-1 text-[13px] leading-5 text-slate-500">{meta.description}</div>
              </div>
              <StatusPill {...desktopStatus} ariaLabel={`网络状态${desktopStatus.label}`} />
            </header>
            <section className="min-h-0 flex-1 overflow-auto bg-white">
              <Outlet />
            </section>
          </main>
        </div>
      </TooltipProvider>
    );
  }

  // 浏览器宿主：状态一律来自单一 Runtime 健康事实源，与 Sidebar 完全一致；
  // `/gitea` 额外区分 FileBay 配置状态，不再把浏览器在线写成 FileBay 在线。
  const runtimePill = (() => {
    switch (runtimeStatus) {
      case "online":
        return {
          label: "已连接",
          detail: "Runtime 服务在线",
          tone: "text-emerald-600",
          Icon: Wifi,
          dot: "bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]",
          panel: "border-emerald-100 bg-emerald-50/80",
        };
      case "offline":
        return {
          label: "未连接",
          detail: "请确认服务器 Runtime 已启动，稍后重试",
          tone: "text-red-600",
          Icon: WifiOff,
          dot: "bg-red-400 shadow-[0_0_0_4px_rgba(239,68,68,0.12)]",
          panel: "border-red-100 bg-red-50/80",
        };
      default:
        return {
          label: "正在检测...",
          detail: "正在连接本机 Runtime",
          tone: "text-slate-600",
          Icon: Wifi,
          dot: "bg-slate-400 shadow-[0_0_0_4px_rgba(148,163,184,0.12)]",
          panel: "border-slate-100 bg-slate-50/80",
        };
    }
  })();

  let filebayPill: Omit<StatusPillProps, "onRetry"> | null = null;
  if (isGiteaSettings) {
    if (filebayConfig.kind === "checking") {
      filebayPill = {
        label: "FileBay 检测中...",
        detail: "读取配置状态",
        tone: "text-slate-600",
        Icon: Globe,
        dot: "bg-slate-400 shadow-[0_0_0_4px_rgba(148,163,184,0.12)]",
        panel: "border-slate-100 bg-slate-50/80",
      };
    } else if (filebayConfig.kind === "unreachable") {
      filebayPill = {
        label: "FileBay 状态不可用",
        detail: "无法获取 FileBay 配置状态",
        tone: "text-slate-600",
        Icon: Globe,
        dot: "bg-slate-400 shadow-[0_0_0_4px_rgba(148,163,184,0.12)]",
        panel: "border-slate-100 bg-slate-50/80",
      };
    } else {
      switch (filebayConfig.status) {
        case "configured":
          filebayPill = {
            label: "FileBay 已配置",
            detail: "上传功能已启用",
            tone: "text-emerald-600",
            Icon: Globe,
            dot: "bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]",
            panel: "border-emerald-100 bg-emerald-50/80",
          };
          break;
        case "invalid":
          filebayPill = {
            label: "FileBay 配置无效",
            detail: "请联系管理员检查配置",
            tone: "text-red-600",
            Icon: Globe,
            dot: "bg-red-400 shadow-[0_0_0_4px_rgba(239,68,68,0.12)]",
            panel: "border-red-100 bg-red-50/80",
          };
          break;
        default:
          filebayPill = {
            label: "FileBay 未配置",
            detail: "请联系管理员配置后重启",
            tone: "text-amber-600",
            Icon: Globe,
            dot: "bg-amber-400 shadow-[0_0_0_4px_rgba(245,158,11,0.15)]",
            panel: "border-amber-100 bg-amber-50/70",
          };
      }
    }
  }

  return (
    <TooltipProvider>
      <div className="flex h-screen w-screen overflow-hidden bg-gray-50">
        <Sidebar />
        <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-white">
          <header className="flex h-[120px] shrink-0 items-center justify-between border-b border-slate-200 bg-white px-6">
            <div className="min-w-0 max-w-[760px] pr-6">
              <div className="text-sm font-semibold leading-6 text-slate-900">{headerTitle}</div>
              <div className="mt-1 text-[13px] leading-5 text-slate-500">{meta.description}</div>
            </div>
            <div className="flex shrink-0 items-center gap-3">
              <StatusPill
                {...runtimePill}
                ariaLabel={`网络状态${runtimePill.label}`}
                onRetry={runtimeStatus === "offline" ? () => void refreshRuntime() : undefined}
              />
              {filebayPill && <StatusPill {...filebayPill} ariaLabel={`FileBay 状态${filebayPill.label}`} />}
            </div>
          </header>
          <section className="min-h-0 flex-1 overflow-auto bg-white">
            <Outlet />
          </section>
        </main>
      </div>
    </TooltipProvider>
  );
}
