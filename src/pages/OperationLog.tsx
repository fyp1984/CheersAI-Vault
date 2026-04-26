import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { 
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";
import { useLogStore } from "@/store/logStore";
import { Trash2, RefreshCw, Database, TrendingUp, Clock, FileText } from "lucide-react";
import { cn, formatDate } from "@/lib/utils";
import type { LogLevel } from "@/types/log";
import { useEffect, useState } from "react";
import { tauriCommands } from "@/lib/tauri";
import type { DatabaseStatistics } from "@/types/log";

const levelColor: Record<LogLevel, string> = {
  info: "bg-blue-100 text-blue-600",
  warning: "bg-yellow-100 text-yellow-700",
  error: "bg-red-100 text-red-600",
  success: "bg-green-100 text-green-600",
};

const levelLabel: Record<LogLevel, string> = {
  info: "信息",
  warning: "警告",
  error: "错误",
  success: "成功",
};

export default function OperationLog() {
  const { 
    entries, 
    loading, 
    clearLogs, 
    loadLogs, 
    initializeDatabase,
    currentPage,
    totalPages,
    totalCount,
    setPage
  } = useLogStore();
  const [levelFilter, setLevelFilter] = useState<string>("all");
  const [statistics, setStatistics] = useState<DatabaseStatistics | null>(null);
  const [dbInfo, setDbInfo] = useState<any>(null);

  // 初始化数据库和加载数据
  useEffect(() => {
    const init = async () => {
      try {
        console.log("OperationLog: Initializing...");
        await initializeDatabase();
        console.log("OperationLog: Database initialized");
        await loadStatistics();
        console.log("OperationLog: Statistics loaded");
        await loadDatabaseInfo();
        console.log("OperationLog: Database info loaded");
      } catch (error) {
        console.error("OperationLog: Initialization failed:", error);
      }
    };
    init();
  }, []); // 移除依赖，只在组件挂载时执行一次

  // 加载统计信息
  const loadStatistics = async () => {
    try {
      console.log("Loading statistics...");
      const stats = await tauriCommands.getStatistics();
      console.log("Statistics loaded:", stats);
      setStatistics(stats);
    } catch (error) {
      console.error("Failed to load statistics:", error);
      // 设置默认统计信息，避免页面崩溃
      setStatistics({
        totalFiles: 0,
        successfulFiles: 0,
        failedFiles: 0,
        totalMaskedItems: 0,
        averageProcessingTimeMs: 0,
        recentFiles7days: 0,
        successRate: 0
      });
    }
  };

  // 加载数据库信息
  const loadDatabaseInfo = async () => {
    try {
      console.log("Loading database info...");
      const info = await tauriCommands.getDatabaseInfo();
      console.log("Database info loaded:", info);
      setDbInfo(info);
    } catch (error) {
      console.error("Failed to load database info:", error);
      // 设置默认数据库信息
      setDbInfo({
        status: "error",
        database_path: "unknown",
        database_exists: false,
        log_count: 0
      });
    }
  };

  // 处理级别过滤
  const handleLevelFilter = async (value: string) => {
    setLevelFilter(value);
    setPage(1); // 重置到第一页
    const filter = value === "all" ? undefined : value;
    await loadLogs(1, filter);
  };

  // 刷新日志
  const handleRefresh = async () => {
    const filter = levelFilter === "all" ? undefined : levelFilter;
    await loadLogs(currentPage, filter);
    await loadStatistics();
    await loadDatabaseInfo();
  };

  // 清空日志
  const handleClearLogs = async () => {
    if (confirm("确定要清空所有日志吗？此操作不可撤销。")) {
      await clearLogs();
      await loadStatistics();
    }
  };

  // 处理分页
  const handlePageChange = async (page: number) => {
    setPage(page);
    const filter = levelFilter === "all" ? undefined : levelFilter;
    await loadLogs(page, filter);
  };

  // 生成分页数字
  const generatePageNumbers = () => {
    const pages = [];
    const maxVisible = 5;
    
    if (totalPages <= maxVisible) {
      for (let i = 1; i <= totalPages; i++) {
        pages.push(i);
      }
    } else {
      if (currentPage <= 3) {
        for (let i = 1; i <= 4; i++) {
          pages.push(i);
        }
        pages.push('ellipsis');
        pages.push(totalPages);
      } else if (currentPage >= totalPages - 2) {
        pages.push(1);
        pages.push('ellipsis');
        for (let i = totalPages - 3; i <= totalPages; i++) {
          pages.push(i);
        }
      } else {
        pages.push(1);
        pages.push('ellipsis');
        for (let i = currentPage - 1; i <= currentPage + 1; i++) {
          pages.push(i);
        }
        pages.push('ellipsis');
        pages.push(totalPages);
      }
    }
    
    return pages;
  };

  // 格式化时间
  const formatDuration = (ms: number) => {
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    return `${(ms / 60000).toFixed(1)}min`;
  };

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="操作日志"
        description="查看所有脱敏操作记录和统计信息"
        actions={
          <div className="flex gap-2">
            <Select value={levelFilter} onValueChange={handleLevelFilter}>
              <SelectTrigger className="w-32">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部级别</SelectItem>
                <SelectItem value="info">信息</SelectItem>
                <SelectItem value="success">成功</SelectItem>
                <SelectItem value="warning">警告</SelectItem>
                <SelectItem value="error">错误</SelectItem>
              </SelectContent>
            </Select>
            <Button variant="outline" size="sm" onClick={handleRefresh} disabled={loading}>
              <RefreshCw className={cn("w-4 h-4 mr-1", loading && "animate-spin")} />
              刷新
            </Button>
            <Button variant="outline" size="sm" onClick={handleClearLogs}>
              <Trash2 className="w-4 h-4 mr-1" />
              清空日志
            </Button>
          </div>
        }
      />

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-6xl mx-auto space-y-6">
          
          {/* 统计信息卡片 */}
          {statistics && (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
              <Card>
                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                  <CardTitle className="text-sm font-medium">总处理文件</CardTitle>
                  <FileText className="h-4 w-4 text-muted-foreground" />
                </CardHeader>
                <CardContent>
                  <div className="text-2xl font-bold">{statistics.totalFiles}</div>
                  <p className="text-xs text-muted-foreground">
                    成功率 {statistics.successRate.toFixed(1)}%
                  </p>
                </CardContent>
              </Card>

              <Card>
                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                  <CardTitle className="text-sm font-medium">脱敏项目数</CardTitle>
                  <TrendingUp className="h-4 w-4 text-muted-foreground" />
                </CardHeader>
                <CardContent>
                  <div className="text-2xl font-bold">{statistics.totalMaskedItems.toLocaleString()}</div>
                  <p className="text-xs text-muted-foreground">
                    累计脱敏数据项
                  </p>
                </CardContent>
              </Card>

              <Card>
                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                  <CardTitle className="text-sm font-medium">平均处理时间</CardTitle>
                  <Clock className="h-4 w-4 text-muted-foreground" />
                </CardHeader>
                <CardContent>
                  <div className="text-2xl font-bold">{formatDuration(statistics.averageProcessingTimeMs)}</div>
                  <p className="text-xs text-muted-foreground">
                    单个文件平均时间
                  </p>
                </CardContent>
              </Card>

              <Card>
                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                  <CardTitle className="text-sm font-medium">最近活动</CardTitle>
                  <Database className="h-4 w-4 text-muted-foreground" />
                </CardHeader>
                <CardContent>
                  <div className="text-2xl font-bold">{statistics.recentFiles7days}</div>
                  <p className="text-xs text-muted-foreground">
                    最近7天处理文件
                  </p>
                </CardContent>
              </Card>
            </div>
          )}

          {/* 数据库状态 */}
          {dbInfo && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Database className="w-5 h-5" />
                  数据库状态
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex items-center gap-4 text-sm">
                  <div className="flex items-center gap-2">
                    <div className="w-2 h-2 bg-green-500 rounded-full"></div>
                    <span>状态: {dbInfo.status}</span>
                  </div>
                  <div>日志表: {dbInfo.log_table}</div>
                  <div>初始化时间: {new Date(dbInfo.initialized_at).toLocaleString()}</div>
                </div>
              </CardContent>
            </Card>
          )}

          {/* 日志列表 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                <span>操作日志</span>
                {totalCount > 0 && (
                  <span className="text-sm font-normal text-gray-500">
                    共 {totalCount} 条记录，第 {currentPage} / {totalPages} 页
                  </span>
                )}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                {/* 日志条目 - 固定高度容纳10条记录 */}
                <div className="flex flex-col" style={{ height: '640px' }}>
                  {loading ? (
                    <div className="flex items-center justify-center h-full text-gray-400">
                      <div className="text-center">
                        <RefreshCw className="w-6 h-6 mx-auto mb-2 animate-spin" />
                        <p className="text-sm">加载日志中...</p>
                      </div>
                    </div>
                  ) : entries.length === 0 ? (
                    <div className="flex items-center justify-center h-full text-gray-400">
                      <div className="text-center">
                        <FileText className="w-12 h-12 mx-auto mb-2 opacity-50" />
                        <p className="text-sm">暂无日志记录</p>
                        <p className="text-xs mt-1">开始处理文件后，操作记录将显示在这里</p>
                      </div>
                    </div>
                  ) : (
                    <>
                      {entries.map((entry, index) => {
                        // 计算每条日志的高度：总高度 / 10条记录
                        const itemHeight = 640 / 10;
                        return (
                          <div
                            key={entry.id}
                            className="flex items-start gap-3 px-4 py-3 rounded-lg border border-gray-100 bg-white hover:bg-gray-50"
                            style={{ 
                              height: `${itemHeight}px`,
                              marginBottom: index < entries.length - 1 ? '8px' : '0'
                            }}
                          >
                            <Badge className={cn("text-xs font-normal mt-0.5 shrink-0", levelColor[entry.level])}>
                              {levelLabel[entry.level]}
                            </Badge>
                            <div className="flex-1 min-w-0 overflow-hidden">
                              <p className="text-sm text-gray-800 truncate">{entry.message}</p>
                              {entry.details && (
                                <p className="text-xs text-gray-500 mt-0.5 truncate">{entry.details}</p>
                              )}
                              {entry.filePath && (
                                <p className="text-xs text-blue-600 mt-0.5 font-mono truncate">
                                  文件: {entry.filePath}
                                </p>
                              )}
                              {entry.operationType && (
                                <Badge variant="outline" className="text-xs mt-1">
                                  {entry.operationType}
                                </Badge>
                              )}
                            </div>
                            <span className="text-xs text-gray-400 shrink-0">
                              {formatDate(typeof entry.timestamp === 'string' 
                                ? new Date(entry.timestamp).getTime() 
                                : entry.timestamp)}
                            </span>
                          </div>
                        );
                      })}
                      
                      {/* 填充剩余空间 */}
                      {entries.length < 10 && (
                        <>
                          {Array.from({ length: 10 - entries.length }).map((_, index) => {
                            const itemHeight = 640 / 10;
                            const actualIndex = entries.length + index;
                            return (
                              <div
                                key={`placeholder-${index}`}
                                className="rounded-lg border border-gray-50 bg-gray-25 opacity-30"
                                style={{ 
                                  height: `${itemHeight}px`,
                                  marginBottom: actualIndex < 9 ? '8px' : '0'
                                }}
                              />
                            );
                          })}
                        </>
                      )}
                    </>
                  )}
                </div>

                {/* 分页组件 */}
                {totalPages > 1 && (
                  <div className="flex justify-center pt-4 border-t">
                    <Pagination>
                      <PaginationContent>
                        <PaginationItem>
                          <PaginationPrevious 
                            onClick={() => handlePageChange(currentPage - 1)}
                            className={cn(
                              currentPage <= 1 && "pointer-events-none opacity-50"
                            )}
                          />
                        </PaginationItem>
                        
                        {generatePageNumbers().map((page, index) => (
                          <PaginationItem key={index}>
                            {page === 'ellipsis' ? (
                              <PaginationEllipsis />
                            ) : (
                              <PaginationLink
                                onClick={() => handlePageChange(page as number)}
                                isActive={currentPage === page}
                              >
                                {page}
                              </PaginationLink>
                            )}
                          </PaginationItem>
                        ))}
                        
                        <PaginationItem>
                          <PaginationNext 
                            onClick={() => handlePageChange(currentPage + 1)}
                            className={cn(
                              currentPage >= totalPages && "pointer-events-none opacity-50"
                            )}
                          />
                        </PaginationItem>
                      </PaginationContent>
                    </Pagination>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
