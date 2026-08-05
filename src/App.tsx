import { HashRouter, Routes, Route, Navigate, useNavigate } from "react-router-dom";
import { useEffect, lazy, Suspense } from "react";
import { MainLayout } from "@/components/layout/MainLayout";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import FileProcess from "@/pages/FileProcess";
import FileUnmask from "@/pages/FileUnmask";
import SensitiveTerms from "@/pages/SensitiveTerms";
import OperationLog from "@/pages/OperationLog";
import CheersAICloudBrowser from "@/pages/CheersAICloudBrowser";
import { FileManager } from "@/components/file/FileManager";
import EnhancedServices from "@/pages/EnhancedServices";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

// 桌面（Tauri）宿主专属的启动编排（平台上下文/旧库迁移/数据库初始化/导航事件监听/
// 配置同步调试工具挂载）集中于此模块；普通浏览器懒加载入口不下载、不执行本模块。
const DesktopBootstrap = lazy(() => import("@/components/runtime/DesktopBootstrap"));

// 以下三个页面当前内容仍直接调用 Tauri 命令（只读边界确认，本任务未改动其内容），
// 改为路由级懒加载以确保普通浏览器不下载其静态闭包。
const SandboxManager = lazy(() => import("@/pages/SandboxManager"));
const GiteaSettings = lazy(() =>
  import("@/components/settings/GiteaSettings").then((m) => ({ default: m.GiteaSettings }))
);

const lazyRouteFallback = (
  <div className="flex h-full items-center justify-center">
    <Loading text="正在加载…" />
  </div>
);

function AppRoutes() {
  const navigate = useNavigate();
  // 单一宿主判定入口：普通浏览器不得下载/触发下面任何 Tauri invoke/事件监听。
  const isDesktop = isTauriHost();

  useEffect(() => {
    document.title = isDesktop
      ? "CheersAI Vault · 智享AI，安全随行"
      : "CheersAI Vault Pro · 内网工作区";
  }, [isDesktop]);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const target = params.get("target");

    if (target === "process") {
      navigate("/process", { replace: true });
      window.history.replaceState(
        null,
        "",
        `${window.location.pathname}${window.location.hash}`
      );
    }
  }, [navigate]);

  return (
    <>
      {isDesktop && (
        <Suspense fallback={null}>
          <DesktopBootstrap />
        </Suspense>
      )}
      <Routes>
        <Route element={<MainLayout />}>
          <Route index element={<HomeRedirect />} />
          <Route path="/process" element={<ErrorBoundary><FileProcess /></ErrorBoundary>} />
          <Route path="/unmask" element={<FileUnmask />} />
          <Route path="/files" element={<FileManager />} />
          <Route path="/gitea" element={<Suspense fallback={lazyRouteFallback}><GiteaSettings /></Suspense>} />
          <Route path="/rules" element={<Navigate to="/sensitive-terms" replace />} />
          <Route path="/sensitive-terms" element={<SensitiveTerms />} />
          <Route path="/sandbox" element={<Suspense fallback={lazyRouteFallback}><SandboxManager /></Suspense>} />
          <Route path="/log" element={<OperationLog />} />
          <Route path="/cloud" element={<CheersAICloudBrowser />} />
          <Route path="/enhanced" element={<EnhancedServices />} />
        </Route>
      </Routes>
    </>
  );
}

function HomeRedirect() {
  const target = new URLSearchParams(window.location.search).get("target");
  if (target === "process") {
    return <Navigate to="/process" replace />;
  }
  return <Navigate to="/cloud" replace />;
}

function App() {
  console.log("App component loaded");
  
  return (
    <HashRouter>
      <AppRoutes />
    </HashRouter>
  );
}

export default App;
