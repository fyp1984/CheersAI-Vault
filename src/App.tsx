import { HashRouter, Routes, Route, Navigate, useNavigate } from "react-router-dom";
import { useEffect } from "react";
import { listen } from '@tauri-apps/api/event';
import { MainLayout } from "@/components/layout/MainLayout";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import FileProcess from "@/pages/FileProcess";
import FileUnmask from "@/pages/FileUnmask";
import SensitiveTerms from "@/pages/SensitiveTerms";
import SandboxManager from "@/pages/SandboxManager";
import OperationLog from "@/pages/OperationLog";
import CheersAICloudBrowser from "@/pages/CheersAICloudBrowser";
import TestPage from "@/pages/TestPage";
import { FileManager } from "@/components/file/FileManager";
import { GiteaSettings } from "@/components/settings/GiteaSettings";
import { EnhancedServices } from "@/pages/EnhancedServices";
import { InstallerTest } from "@/pages/InstallerTest";
import { useLogStore } from "@/store/logStore";
import { tauriCommands } from "@/lib/tauri";
import { setPlatformContext } from "@/lib/path";
// 导入配置同步工具，使其在开发者控制台中可用
import '@/lib/sync-config';

function AppRoutes() {
  const { initializeDatabase } = useLogStore();
  const navigate = useNavigate();

  useEffect(() => {
    document.title = "CheersAI Desktop · 智享AI，安全随行";
  }, []);

  useEffect(() => {
    const bootstrapPlatformContext = async () => {
      try {
        const context = await tauriCommands.getPlatformContext();
        setPlatformContext(context);
      } catch (error) {
        console.error("Failed to load platform context:", error);
      }
    };

    bootstrapPlatformContext();
  }, []);

  // 应用启动时初始化数据库和迁移旧数据
  useEffect(() => {
    const init = async () => {
      try {
        // 先尝试迁移旧数据库
        try {
          const migrationResult = await tauriCommands.migrateOldDatabase();
          console.log("Migration result:", migrationResult);
        } catch (migrationError) {
          console.log("No migration needed or migration failed:", migrationError);
        }
        
        // 然后初始化数据库
        await initializeDatabase();
        console.log("Database initialized successfully");
      } catch (error) {
        console.error("Failed to initialize database:", error);
      }
    };
    init();
  }, [initializeDatabase]);

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

  // 监听导航事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    
    const setupListener = async () => {
      try {
        unlisten = await listen('navigate-to-process', () => {
          console.log('Received navigate-to-process event');
          navigate('/process');
        });
      } catch (error) {
        console.error('Failed to setup event listener:', error);
      }
    };
    
    setupListener();
    
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [navigate]);

  return (
    <Routes>
      <Route element={<MainLayout />}>
        <Route index element={<HomeRedirect />} />
        <Route path="/test" element={<TestPage />} />
        <Route path="/process" element={<ErrorBoundary><FileProcess /></ErrorBoundary>} />
        <Route path="/unmask" element={<FileUnmask />} />
        <Route path="/files" element={<FileManager />} />
        <Route path="/gitea" element={<GiteaSettings />} />
        <Route path="/rules" element={<Navigate to="/sensitive-terms" replace />} />
        <Route path="/sensitive-terms" element={<SensitiveTerms />} />
        <Route path="/sandbox" element={<SandboxManager />} />
        <Route path="/log" element={<OperationLog />} />
        <Route path="/cloud" element={<CheersAICloudBrowser />} />
        <Route path="/enhanced" element={<EnhancedServices />} />
        <Route path="/installer-test" element={<InstallerTest />} />
      </Route>
    </Routes>
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
