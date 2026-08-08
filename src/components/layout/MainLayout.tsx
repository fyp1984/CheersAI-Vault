import { Outlet, useLocation } from "react-router-dom";
import { useEffect, useState } from "react";
import { Wifi, WifiOff } from "lucide-react";
import { Sidebar } from "./Sidebar";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { isTauriHost } from "@/lib/runtime/host";

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
  "/sensitive-terms": {
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

interface StatusPillProps {
  label: string;
  detail: string;
  tone: string;
  Icon: typeof Wifi;
  dot: string;
  panel: string;
  ariaLabel?: string;
}

function StatusPill({ label, detail, tone, Icon, dot, panel, ariaLabel }: StatusPillProps) {
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
        </div>
        <div className="truncate text-xs text-slate-500">{detail}</div>
      </div>
    </div>
  );
}

export function MainLayout() {
  const location = useLocation();
  const isDesktopHost = isTauriHost();
  const isCloudWorkspace = location.pathname.startsWith("/cloud");
  const meta = pageMeta[location.pathname] ?? {
    title: isDesktopHost ? "CheersAI Vault" : "CheersAI Vault Pro",
    description: "把敏感数据留在本地，让AI能力触手可及。",
  };
  // 浏览器宿主页头不得出现 "CheersAI Desktop"（该名称仅指智能体工作台，
  // 不得用于本项目浏览器版）；桌面宿主 `/cloud` 内嵌的正是智能体工作台
  // 子 WebView，保留其原名属已确认的指向性用法。
  const headerTitle = isCloudWorkspace
    ? (isDesktopHost ? "CheersAI Desktop 在线工作区" : "CheersAI 云端工作区")
    : meta.title;
  const [internetConnected, setInternetConnected] = useState(() => {
    if (typeof navigator === "undefined") {
      return false;
    }
    return navigator.onLine;
  });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const handleOnline = () => setInternetConnected(true);
    const handleOffline = () => setInternetConnected(false);

    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, []);

  const currentMenuNetworkConnected = isCloudWorkspace && internetConnected;
  const networkPill = currentMenuNetworkConnected
    ? {
        label: "已连接",
        detail: "互联网连接状态",
        tone: "text-emerald-600",
        Icon: Wifi,
        dot: "bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]",
        panel: "border-emerald-100 bg-emerald-50/80",
      }
    : {
        label: "已离线",
        detail: "互联网连接状态",
        tone: "text-slate-600",
        Icon: WifiOff,
        dot: "bg-slate-400 shadow-[0_0_0_4px_rgba(148,163,184,0.12)]",
        panel: "border-slate-100 bg-slate-50/80",
      };
  const networkTooltip = currentMenuNetworkConnected
    ? "当前可连通互联网"
    : "当前与互联网断开";

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
            <Tooltip>
              <TooltipTrigger asChild>
                <div>
                  <StatusPill {...networkPill} ariaLabel={`网络状态${networkPill.label}`} />
                </div>
              </TooltipTrigger>
              <TooltipContent side="bottom">{networkTooltip}</TooltipContent>
            </Tooltip>
          </header>
          <section className="min-h-0 flex-1 overflow-auto bg-white">
            <Outlet />
          </section>
        </main>
      </div>
    </TooltipProvider>
  );
}
