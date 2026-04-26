import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { LogEntry, LogLevel } from "@/types/log";
import { tauriCommands } from "@/lib/tauri";

interface LogStore {
  entries: LogEntry[];
  loading: boolean;
  initialized: boolean;
  currentPage: number;
  totalPages: number;
  pageSize: number;
  totalCount: number;
  addLog: (level: LogLevel, message: string, details?: string, filePath?: string, operationType?: string) => Promise<void>;
  loadLogs: (page?: number, levelFilter?: string) => Promise<void>;
  clearLogs: () => Promise<void>;
  initializeDatabase: () => Promise<void>;
  setPage: (page: number) => void;
  getTotalCount: (levelFilter?: string) => Promise<number>;
}

export const useLogStore = create<LogStore>()(
  persist(
    (set, get) => ({
      entries: [],
      loading: false,
      initialized: false,
      currentPage: 1,
      totalPages: 0,
      pageSize: 10,
      totalCount: 0,

      initializeDatabase: async () => {
        try {
          console.log("Initializing database...");
          await tauriCommands.initializeDatabase();
          console.log("Database initialized successfully");
          set({ initialized: true });
          // 初始化后加载第一页日志
          await get().loadLogs(1);
          console.log("Initial logs loaded");
        } catch (error) {
          console.error("Failed to initialize database:", error);
          // 即使数据库初始化失败，也标记为已初始化，避免重复尝试
          set({ initialized: true });
        }
      },

      addLog: async (level, message, details, filePath, operationType) => {
        try {
          console.log(`Adding log: ${level} - ${message}`);
          // 添加到数据库
          await tauriCommands.addLogEntry(level, message, details, filePath, operationType);
          console.log("Log added to database successfully");
          
          // 如果当前在第一页，重新加载以显示新日志
          const { currentPage } = get();
          if (currentPage === 1) {
            await get().loadLogs(1);
          }
        } catch (error) {
          console.error("Failed to add log to database:", error);
          
          // 如果数据库失败，至少添加到内存中作为备用
          const entry: LogEntry = {
            id: Date.now().toString(),
            timestamp: Date.now(),
            level,
            message,
            details,
            filePath,
            operationType,
          };
          
          console.log("Adding log to memory as fallback");
          set((state) => ({
            entries: [entry, ...state.entries].slice(0, state.pageSize),
          }));
        }
      },

      getTotalCount: async (levelFilter?: string) => {
        try {
          return await tauriCommands.getLogsCount(levelFilter);
        } catch (error) {
          console.error("Failed to get total count:", error);
          return 0;
        }
      },

      loadLogs: async (page = 1, levelFilter) => {
        if (!get().initialized) {
          console.log("Database not initialized, initializing now...");
          await get().initializeDatabase();
        }
        
        console.log(`Loading logs: page=${page}, filter=${levelFilter}`);
        set({ loading: true });
        try {
          const { pageSize } = get();
          const offset = (page - 1) * pageSize;
          
          // 获取总数
          console.log("Getting total count...");
          const totalCount = await get().getTotalCount(levelFilter);
          console.log(`Total count: ${totalCount}`);
          const totalPages = Math.ceil(totalCount / pageSize);
          
          // 获取当前页数据
          console.log(`Getting logs: limit=${pageSize}, offset=${offset}`);
          const logs = await tauriCommands.getLogs(pageSize, offset, levelFilter);
          console.log(`Retrieved ${logs.length} logs from database`);
          
          // 转换时间戳格式
          const processedLogs = logs.map(log => ({
            ...log,
            timestamp: typeof log.timestamp === 'string' 
              ? new Date(log.timestamp).getTime() 
              : log.timestamp,
            filePath: log.filePath || undefined,
            operationType: log.operationType || undefined,
          }));
          
          console.log(`Processed logs:`, processedLogs);
          
          set({ 
            entries: processedLogs, 
            loading: false,
            currentPage: page,
            totalPages,
            totalCount
          });
        } catch (error) {
          console.error("Failed to load logs from database:", error);
          set({ loading: false });
          
          // 如果数据库加载失败，显示空状态
          set({
            entries: [],
            currentPage: page,
            totalPages: 0,
            totalCount: 0
          });
        }
      },

      setPage: (page: number) => {
        set({ currentPage: page });
      },

      clearLogs: async () => {
        try {
          await tauriCommands.clearAllLogs();
          set({ 
            entries: [], 
            currentPage: 1, 
            totalPages: 0, 
            totalCount: 0 
          });
        } catch (error) {
          console.error("Failed to clear logs:", error);
          // 如果数据库清除失败，至少清除内存中的
          set({ 
            entries: [], 
            currentPage: 1, 
            totalPages: 0, 
            totalCount: 0 
          });
        }
      },
    }),
    {
      name: "log-store",
      // 只持久化初始化状态和当前页码
      partialize: (state) => ({ 
        initialized: state.initialized,
        currentPage: state.currentPage 
      }),
    }
  )
);
