import { Outlet, useLocation } from "react-router-dom";
import { Wifi, WifiOff, Globe } from "lucide-react";
import { Sidebar } from "./Sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";

const pageMeta: Record<string, { title: string; description: string }> = {
  "/cloud": {
    title: "Desktop 在线工作区",
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

export function MainLayout() {
  const location = useLocation();
  const isDesktopWorkspace = location.pathname.startsWith("/cloud");
  const isGiteaSettings = location.pathname.startsWith("/gitea");
  const meta = pageMeta[location.pathname] ?? {
    title: "CheersAI Desktop",
    description: "把敏感数据留在本地，让AI能力触手可及。",
  };
  
  // 根据页面类型确定网络状态
  const networkStatus = (isDesktopWorkspace || isGiteaSettings)
    ? {
        label: "在线",
        detail: isDesktopWorkspace ? "当前操作 Desktop 在线工作区" : "当前操作 FileBay 在线服务",
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
              <div className="text-sm font-semibold leading-6 text-slate-900">{meta.title}</div>
              <div className="mt-1 text-[13px] leading-5 text-slate-500">{meta.description}</div>
            </div>

            <div
              className={`flex shrink-0 items-center gap-3 rounded-2xl border px-4 py-2 ${networkStatus.panel}`}
              aria-label={`网络状态${networkStatus.label}`}
            >
              <div className={`h-2.5 w-2.5 rounded-full ${networkStatus.dot}`} />
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <networkStatus.Icon className={`h-4 w-4 ${networkStatus.tone}`} />
                  <span className={`text-sm font-semibold ${networkStatus.tone}`}>{networkStatus.label}</span>
                </div>
                <div className="truncate text-xs text-slate-500">{networkStatus.detail}</div>
              </div>
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
