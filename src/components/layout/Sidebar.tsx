import { NavLink, useLocation } from "react-router-dom";
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
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/store/appStore";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

const navItems = [
  { to: "/cloud", icon: Cloud, label: "CheersAI", description: "访问云端AI服务" },
  { to: "/process", icon: FileText, label: "文件脱敏", description: "处理和脱敏文件" },
  { to: "/unmask", icon: RotateCcw, label: "文件反脱敏", description: "还原已脱敏的文件" },
  { to: "/files", icon: FolderOpen, label: "文件管理", description: "管理脱敏后的文件" },
  { to: "/gitea", icon: Upload, label: "FileBay 设置", description: "配置 FileBay 上传" },
  { to: "/rules", icon: Settings2, label: "规则配置" },
  { to: "/sandbox", icon: Lock, label: "沙箱管理" },
  { to: "/log", icon: ClipboardList, label: "操作日志" },
];



export function Sidebar() {
  const { sidebarCollapsed, toggleSidebar } = useAppStore();
  const location = useLocation();

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
            <div className="truncate text-base font-medium text-white">CheersAI Desktop</div>
            <div className="truncate text-[11px] text-slate-400">智享AI，安全随行</div>
          </div>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-3 py-2 overflow-y-auto">
        {navItems.map(({ to, icon: Icon, label, description }) => {
          const isActive = location.pathname === to || location.pathname.startsWith(to + '/');
          return (
          <Tooltip key={to} delayDuration={0}>
            <TooltipTrigger asChild>
              <NavLink
                to={to}
                className={cn(
                  "flex items-center gap-3 h-12 px-4 mb-1 text-sm rounded-lg transition-all",
                  isActive
                    ? "bg-[#3b82f6] text-white font-medium"
                    : "text-[#d1d5db] hover:bg-white/5"
                )}
                style={{
                  transitionDuration: '200ms'
                }}
              >
                <Icon className="w-5 h-5 shrink-0" />
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
      <div className="px-3 py-4">
        {!sidebarCollapsed && (
          <div className="px-3 py-2 mb-3">
            <div className="flex items-center gap-2 mb-1">
              <div className="w-2 h-2 bg-green-400 rounded-full animate-pulse" />
              <div className="text-xs text-slate-400">系统状态</div>
            </div>
            <div className="text-xs text-slate-300">运行正常</div>
            <div className="text-xs text-slate-500 mt-0.5">版本 v0.1.0</div>
          </div>
        )}
        
        {/* Collapse toggle */}
        <button
          onClick={toggleSidebar}
          className={cn(
            "flex items-center justify-center w-full h-9 rounded-lg transition-all duration-200",
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
