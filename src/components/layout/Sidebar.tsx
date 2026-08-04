import { NavLink, useLocation } from "react-router-dom";
import { useEffect, useState } from "react";
import {
  FileText,
  Settings2,
  Lock,
  ClipboardList,
  ChevronLeft,
  ChevronRight,
  Cloud,
  FolderOpen,
  Upload,
  RotateCcw,
  Sparkles,
  ExternalLink,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { getBuildVersion, getAppVersion } from "@/lib/version";
import { useAppStore } from "@/store/appStore";
import { useRuntimeHealthStore } from "@/store/runtimeStore";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { isTauriHost } from "@/lib/runtime/host";

const HELP_WIKI_URL =
  "https://dcnd0q32i5v3.feishu.cn/wiki/TVChw3onji9mVdkx96tcXsSYnlf?from=from_copylink";

const navItems = [
  { to: "/cloud", icon: Cloud, label: "CheersAI", description: "访问云端AI服务" },
  { to: "/process", icon: FileText, label: "文件脱敏", description: "处理和脱敏文件" },
  { to: "/unmask", icon: RotateCcw, label: "文件反脱敏", description: "还原已脱敏的文件" },
  { to: "/files", icon: FolderOpen, label: "文件管理", description: "管理脱敏后的文件" },
  { to: "/gitea", icon: Upload, label: "FileBay 设置", description: "配置 FileBay 上传" },
  { to: "/enhanced", icon: Sparkles, label: "增强服务", description: "安装 OCR 等增强功能" },
  { to: "/sensitive-terms", icon: Settings2, label: "规则配置" },
  { to: "/sandbox", icon: Lock, label: "沙箱管理" },
  { to: "/log", icon: ClipboardList, label: "操作日志" },
];



export function Sidebar() {
  const { sidebarCollapsed, toggleSidebar, activePreviewId } = useAppStore();
  const location = useLocation();
  const [appVersion, setAppVersion] = useState(`v${getBuildVersion()}`);
  const [hoveredItem, setHoveredItem] = useState<string | null>(null);
  const isDesktop = isTauriHost();
  // 浏览器宿主：底部"Runtime 状态"来自单一事实源 store（与 MainLayout 同源），
  // 不得各自本地健康检查；桌面宿主保持既有"运行正常"语义。
  const browserRuntimeStatus = useRuntimeHealthStore((state) => state.status);

  useEffect(() => {
    let active = true;

    void getAppVersion().then((version) => {
      if (active) {
        setAppVersion(`v${version}`);
      }
    });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    setHoveredItem(null);
  }, [sidebarCollapsed]);

  const handleOpenHelpWiki = async () => {
    // 浏览器宿主直接用标准浏览器能力打开，不尝试任何 Tauri 调用。
    if (!isDesktop) {
      window.open(HELP_WIKI_URL, "_blank", "noopener,noreferrer");
      return;
    }

    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(HELP_WIKI_URL);
    } catch (error) {
      console.error("Failed to open help wiki:", error);
      window.open(HELP_WIKI_URL, "_blank", "noopener,noreferrer");
    }
  };

  return (
    <aside
      className={cn(
        "flex flex-col h-full text-white transition-all shrink-0",
        sidebarCollapsed ? "w-16" : "w-64"
      )}
      style={{
        background: 'linear-gradient(180deg, #111827 0%, #1f2937 100%)',
        transitionDuration: '200ms'
      }}
    >
      {/* Logo */}
      <div className="flex items-center gap-3 px-4 py-4 border-b border-white/10">
        <img src="/logo.jpg" alt="Logo" className="w-8 h-8 rounded-lg shrink-0" />
        {!sidebarCollapsed && (
          <div className="min-w-0">
            <div className="truncate text-base font-medium text-white">
              {isDesktop ? "CheersAI Vault" : "CheersAI Vault Pro"}
            </div>
            <div className="truncate text-[11px] text-slate-400">智享AI，安全随行</div>
          </div>
        )}
      </div>

      {/* Navigation */}
      <nav className={cn("flex-1 overflow-y-auto", sidebarCollapsed ? "px-2 py-3" : "px-3 py-2")}>
        {navItems.map(({ to, icon: Icon, label, description }) => {
          // 浏览器会话中存在活动 preview 时，"文件脱敏"入口恢复到该 preview，
          // 避免切页后丢失活动预览入口；其余链接保持固定路径。
          const resolvedTo =
            to === "/process" && activePreviewId ? `/process?preview=${encodeURIComponent(activePreviewId)}` : to;
          const isActive = location.pathname === to || location.pathname.startsWith(to + '/');
          return (
          <Tooltip
            key={to}
            delayDuration={100}
            open={sidebarCollapsed && hoveredItem === to}
          >
            <TooltipTrigger asChild>
              <NavLink
                to={resolvedTo}
                onMouseEnter={() => setHoveredItem(to)}
                onMouseLeave={() => setHoveredItem(null)}
                onFocus={() => setHoveredItem(to)}
                onBlur={() => setHoveredItem(null)}
                className={cn(
                  "mb-1 flex items-center text-sm transition-all active:scale-95",
                  sidebarCollapsed
                    ? "mx-auto h-11 w-11 justify-center rounded-xl px-0"
                    : "h-12 gap-3 rounded-lg px-4",
                  isActive
                    ? sidebarCollapsed
                      ? "bg-[#3b82f6] text-white shadow-lg shadow-blue-950/20"
                      : "bg-[#3b82f6] text-white font-medium"
                    : "text-[#d1d5db] hover:bg-white/5 hover:text-white"
                )}
                style={{
                  transitionDuration: '200ms'
                }}
              >
                <Icon className={cn("shrink-0", sidebarCollapsed ? "h-5.5 w-5.5" : "h-5 w-5")} />
                {!sidebarCollapsed && (
                  <span>{label}</span>
                )}
              </NavLink>
            </TooltipTrigger>
            {sidebarCollapsed && (
              <TooltipContent 
                side="right" 
                className="bg-slate-800 text-white border-slate-700 shadow-xl"
                sideOffset={10}
                onPointerDownOutside={() => setHoveredItem(null)}
              >
                <div className="flex flex-col">
                  <span className="font-medium">{label}</span>
                  {description && (
                    <span className="text-xs text-slate-400">{description}</span>
                  )}
                </div>
              </TooltipContent>
            )}
          </Tooltip>
        );
        })}
      </nav>

      {/* Footer */}
      <div className={cn("py-4", sidebarCollapsed ? "px-2" : "px-3")}>
        {!sidebarCollapsed && (
          <div className="px-3 py-2 mb-3 space-y-3">
            <div>
              {isDesktop ? (
                <>
                  <div className="flex items-center gap-2 mb-1">
                    <div className="w-2 h-2 bg-green-400 rounded-full animate-pulse" />
                    <div className="text-xs text-slate-400">系统状态</div>
                  </div>
                  <div className="text-xs text-slate-300">运行正常</div>
                </>
              ) : (
                <>
                  <div className="flex items-center gap-2 mb-1">
                    <div
                      className={cn(
                        "w-2 h-2 rounded-full",
                        browserRuntimeStatus === "online" && "bg-green-400 animate-pulse",
                        browserRuntimeStatus === "offline" && "bg-red-400",
                        browserRuntimeStatus === "checking" && "bg-slate-500 animate-pulse"
                      )}
                    />
                    <div className="text-xs text-slate-400">Runtime 状态</div>
                  </div>
                  <div className="text-xs text-slate-300">
                    {browserRuntimeStatus === "online" && "已连接"}
                    {browserRuntimeStatus === "offline" && "未连接，请确认服务器 Runtime 已启动"}
                    {browserRuntimeStatus === "checking" && "正在检测..."}
                  </div>
                </>
              )}
              <div className="text-xs text-slate-500 mt-0.5">版本 {appVersion}</div>
            </div>
            <button
              type="button"
              onClick={handleOpenHelpWiki}
              className="flex h-9 w-full items-center justify-center gap-2 rounded-lg border border-white/10 bg-white/5 text-xs font-medium text-slate-200 transition-all hover:border-blue-400/50 hover:bg-blue-500/20 hover:text-white active:scale-95"
              aria-label="打开使用文档"
            >
              <ExternalLink className="h-4 w-4" />
              使用说明
            </button>
          </div>
        )}
        
        {/* Collapse toggle */}
        <button
          onClick={toggleSidebar}
          className={cn(
            "flex items-center justify-center rounded-xl transition-all duration-200",
            sidebarCollapsed ? "mx-auto h-11 w-11" : "h-9 w-full",
            "text-slate-400 hover:text-white hover:bg-slate-600/20",
            "active:scale-95"
          )}
        >
          <div className="flex items-center gap-2">
            {sidebarCollapsed ? (
              <ChevronRight className="w-4 h-4" />
            ) : (
              <>
                <ChevronLeft className="w-4 h-4" />
                <span className="text-xs font-normal">收起</span>
              </>
            )}
          </div>
        </button>
      </div>
    </aside>
  );
}
