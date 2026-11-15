import { useEffect, useRef, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Message } from "@/components/ui/cheersai-ui";
import ConfirmDialog from "@/components/common/ConfirmDialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";
import { Trash2, RefreshCw, Database, TrendingUp, Clock, FileText, AlertTriangle, Search } from "lucide-react";
import { cn, formatDate } from "@/lib/utils";
import {
  clearRuntimeOperationLogs,
  fetchRuntimeOperationLogStatistics,
  fetchRuntimeOperationLogStorageStatus,
  fetchRuntimeOperationLogs,
} from "@/lib/runtime/client";
import type { RuntimeFetchResult } from "@/lib/runtime/client";
import type {
  RuntimeOperationLogEntry,
  RuntimeOperationLogLevel,
  RuntimeOperationLogListResponse,
  RuntimeOperationLogStatistics,
  RuntimeOperationLogStorageStatus,
} from "@/types/runtime";

const PAGE_SIZE = 10;

const levelColor: Record<RuntimeOperationLogLevel, string> = {
  info: "bg-blue-100 text-blue-600",
  success: "bg-blue-100 text-blue-600",
  warning: "bg-yellow-100 text-yellow-700",
  error: "bg-red-100 text-red-600",
};

const levelLabel: Record<RuntimeOperationLogLevel, string> = {
  info: "信息",
  success: "成功",
  warning: "警告",
  error: "错误",
};

/**
 * 事件类型到固定安全中文文案的单一映射（§5）——绝不使用数据库中的自由
 * 文本，未知历史事件类型统一降级为通用"状态事件"文案，不导致渲染异常。
 */
const eventTypeLabel: Record<string, string> = {
  Queued: "已加入队列",
  ProcessingStarted: "开始处理",
  Completed: "处理成功",
  Failed: "处理失败",
  RetryQueued: "已重新排队",
  RecoveredInterrupted: "处理已恢复（此前中断）",
  RestoreSucceeded: "反脱敏成功",
  RestoreFailed: "反脱敏失败",
};

function describeEvent(entry: RuntimeOperationLogEntry): string {
  return eventTypeLabel[entry.event_type] ?? "状态事件";
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}min`;
}

const CONNECTION_ERROR_TEXT = "无法连接本机 Runtime，请确认服务已启动后重试。";
const INVALID_QUERY_ERROR_TEXT = "日志查询参数无效，请检查筛选条件后重试。";
const STORAGE_ERROR_TEXT = "日志存储操作失败，请稍后重试。";
const HTTP_ERROR_TEXT = "日志请求失败，请稍后重试。";

type RuntimeFailure = Extract<RuntimeFetchResult<unknown>, { ok: false }>;
type OperationLogRequestScope = { level: string; status: string; batchId: string };

function describeRuntimeFailure(result: RuntimeFailure): string {
  if (result.reason === "network" || result.reason === "parse") {
    return CONNECTION_ERROR_TEXT;
  }
  if (result.code === "INVALID_QUERY") {
    return INVALID_QUERY_ERROR_TEXT;
  }
  if (result.code === "STORAGE_INTERNAL_ERROR") {
    return STORAGE_ERROR_TEXT;
  }
  return HTTP_ERROR_TEXT;
}

export default function OperationLogBrowser() {
  const [entries, setEntries] = useState<RuntimeOperationLogEntry[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [totalPages, setTotalPages] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  const [stats, setStats] = useState<RuntimeOperationLogStatistics | null>(null);
  const [storageStatus, setStorageStatus] = useState<RuntimeOperationLogStorageStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [hasLoadedOnce, setHasLoadedOnce] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [clearing, setClearing] = useState(false);
  const [clearMessage, setClearMessage] = useState<string | null>(null);
  const [confirmClearOpen, setConfirmClearOpen] = useState(false);

  const [levelFilter, setLevelFilter] = useState<string>("all");
  const [statusInput, setStatusInput] = useState("");
  const [batchIdInput, setBatchIdInput] = useState("");
  const [appliedStatus, setAppliedStatus] = useState("");
  const [appliedBatchId, setAppliedBatchId] = useState("");
  const requestGenerationRef = useRef(0);
  const latestScopeRef = useRef<OperationLogRequestScope>({ level: "all", status: "", batchId: "" });
  latestScopeRef.current = { level: levelFilter, status: appliedStatus, batchId: appliedBatchId };

  const loadData = async (page: number, scope = latestScopeRef.current) => {
    const requestGeneration = ++requestGenerationRef.current;
    setLoading(true);
    setConnectionError(null);
    try {
      const [listResult, statsResult, storageResult] = await Promise.all([
        fetchRuntimeOperationLogs({
          page,
          pageSize: PAGE_SIZE,
          level: scope.level === "all" ? undefined : scope.level,
          status: scope.status || undefined,
          batchId: scope.batchId || undefined,
        }),
        fetchRuntimeOperationLogStatistics(),
        fetchRuntimeOperationLogStorageStatus(),
      ]);

      if (requestGeneration !== requestGenerationRef.current) return;
      if (!listResult.ok) {
        setConnectionError(describeRuntimeFailure(listResult));
        return;
      }
      if (!statsResult.ok) {
        setConnectionError(describeRuntimeFailure(statsResult));
        return;
      }
      if (!storageResult.ok) {
        setConnectionError(describeRuntimeFailure(storageResult));
        return;
      }

      const list: RuntimeOperationLogListResponse = listResult.data;
      setEntries(list.entries);
      setTotalCount(list.total_count);
      setTotalPages(list.total_pages);
      setCurrentPage(list.page);
      setStats(statsResult.data);
      setStorageStatus(storageResult.data);
      setConnectionError(null);
      setHasLoadedOnce(true);
    } finally {
      if (requestGeneration === requestGenerationRef.current) {
        setLoading(false);
      }
    }
  };

   
  useEffect(() => {
    void loadData(1);
  }, [levelFilter, appliedStatus, appliedBatchId]);

  const handleRefresh = () => {
    void loadData(currentPage);
  };

  const handlePageChange = (page: number) => {
    if (page < 1 || (totalPages > 0 && page > totalPages)) return;
    void loadData(page);
  };

  const handleSearch = () => {
    setAppliedStatus(statusInput.trim());
    setAppliedBatchId(batchIdInput.trim());
  };

  const handleClear = async () => {
    setClearing(true);
    try {
      const result = await clearRuntimeOperationLogs();
      if (!result.ok) {
        setClearMessage(describeRuntimeFailure(result));
        return;
      }
      setClearMessage(
        `操作日志已清空，共删除 ${result.data.deleted_job_events + result.data.deleted_restore_events} 条记录。`
      );
      await loadData(1);
    } finally {
      setClearing(false);
      setConfirmClearOpen(false);
    }
  };

  const isConnectionError = connectionError === CONNECTION_ERROR_TEXT;
  const showFullPageError = isConnectionError && !hasLoadedOnce;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="操作日志"
        description="查看所有脱敏操作记录和统计信息"
        actions={
          <div className="flex gap-2">
            <Select value={levelFilter} onValueChange={setLevelFilter}>
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
            <Button variant="outline" size="sm" onClick={() => setConfirmClearOpen(true)} disabled={clearing}>
              <Trash2 className="w-4 h-4 mr-1" />
              清空日志
            </Button>
          </div>
        }
      />

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-6xl mx-auto space-y-6">
          {showFullPageError ? (
            <Card>
              <CardContent className="py-10 text-center space-y-3">
                <AlertTriangle className="w-8 h-8 mx-auto text-red-500" />
                <p className="text-sm text-gray-700">{connectionError}</p>
                <Button variant="outline" size="sm" onClick={handleRefresh}>
                  <RefreshCw className="w-4 h-4 mr-1" />
                  重试
                </Button>
              </CardContent>
            </Card>
          ) : (
            <>
              {connectionError && (
                <Message
                  type="warning"
                  title={isConnectionError ? "连接暂时中断" : "暂时无法更新日志"}
                >
                  <div className="flex items-center justify-between gap-4">
                    <span>
                      {isConnectionError
                        ? `当前先显示上一次成功获取的数据。${connectionError}`
                        : connectionError}
                    </span>
                    <Button variant="outline" size="sm" onClick={handleRefresh}>
                      重新加载
                    </Button>
                  </div>
                </Message>
              )}

              {clearMessage && (
                <Message type="success" onClose={() => setClearMessage(null)}>
                  {clearMessage}
                </Message>
              )}

              {stats && (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                  <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                      <CardTitle className="text-sm font-medium">总处理文件</CardTitle>
                      <FileText className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                      <div className="text-2xl font-bold">{stats.total_files}</div>
                      <p className="text-xs text-muted-foreground">成功率 {stats.success_rate.toFixed(1)}%</p>
                    </CardContent>
                  </Card>

                  <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                      <CardTitle className="text-sm font-medium">脱敏项目数</CardTitle>
                      <TrendingUp className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                      <div className="text-2xl font-bold">{stats.total_masked_items.toLocaleString()}</div>
                      <p className="text-xs text-muted-foreground">累计脱敏数据项</p>
                    </CardContent>
                  </Card>

                  <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                      <CardTitle className="text-sm font-medium">平均处理时间</CardTitle>
                      <Clock className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                      <div className="text-2xl font-bold">{formatDuration(stats.average_processing_time_ms)}</div>
                      <p className="text-xs text-muted-foreground">单个文件平均时间</p>
                    </CardContent>
                  </Card>

                  <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                      <CardTitle className="text-sm font-medium">最近活动</CardTitle>
                      <Database className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                      <div className="text-2xl font-bold">{stats.recent_files_7days}</div>
                      <p className="text-xs text-muted-foreground">最近7天处理文件</p>
                    </CardContent>
                  </Card>
                </div>
              )}

              {storageStatus && (
                <Card>
                  <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                      <Database className="w-5 h-5" />
                      Runtime 存储状态
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="flex items-center gap-4 text-sm">
                      <div className="flex items-center gap-2">
                        <div
                          className={cn(
                            "w-2 h-2 rounded-full",
                            storageStatus.status === "ready" ? "bg-blue-500" : "bg-red-500"
                          )}
                        ></div>
                        <span>状态: {storageStatus.status}</span>
                      </div>
                      <div>事件数: {storageStatus.event_count}</div>
                      <div>Runtime 版本: {storageStatus.runtime_version}</div>
                    </div>
                  </CardContent>
                </Card>
              )}

              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center justify-between">
                    <span>操作日志</span>
                    {totalCount > 0 && (
                      <span className="text-sm font-normal text-gray-500">
                        共 {totalCount} 条记录，第 {currentPage} / {Math.max(totalPages, 1)} 页
                      </span>
                    )}
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-4">
                    <div className="flex gap-2 flex-wrap">
                      <Input
                        placeholder="状态筛选，如 Completed"
                        value={statusInput}
                        onChange={(event) => setStatusInput(event.target.value)}
                        className="w-48"
                      />
                      <Input
                        placeholder="批次 ID 前缀"
                        value={batchIdInput}
                        onChange={(event) => setBatchIdInput(event.target.value)}
                        className="w-64"
                      />
                      <Button variant="outline" size="sm" onClick={handleSearch}>
                        <Search className="w-4 h-4 mr-1" />
                        搜索
                      </Button>
                    </div>

                    <div className="space-y-2">
                      {loading ? (
                        <div className="flex items-center justify-center h-40 text-gray-400">
                          <div className="text-center">
                            <RefreshCw className="w-6 h-6 mx-auto mb-2 animate-spin" />
                            <p className="text-sm">正在加载操作日志...</p>
                          </div>
                        </div>
                      ) : entries.length === 0 ? (
                        <div className="flex items-center justify-center h-40 text-gray-400">
                          <div className="text-center">
                            <FileText className="w-12 h-12 mx-auto mb-2 opacity-50" />
                            <p className="text-sm">还没有操作记录</p>
                            <p className="text-xs mt-1">你开始处理文件后，这里会自动显示对应记录。</p>
                          </div>
                        </div>
                      ) : (
                        entries.map((entry) => (
                          <div
                            key={entry.event_id}
                            className="flex items-start gap-3 px-4 py-3 rounded-lg border border-gray-100 bg-white hover:bg-gray-50"
                          >
                            <Badge className={cn("text-xs font-normal mt-0.5 shrink-0", levelColor[entry.level])}>
                              {levelLabel[entry.level]}
                            </Badge>
                            <div className="flex-1 min-w-0 overflow-hidden">
                              <p className="text-sm text-gray-800 truncate">{describeEvent(entry)}</p>
                              <p className="text-xs text-gray-500 mt-0.5 truncate">
                                {entry.display_name && `文件: ${entry.display_name}`}
                                {entry.display_name && entry.masked_entity_count !== null && "　"}
                                {entry.masked_entity_count !== null && `脱敏 ${entry.masked_entity_count} 处`}
                                {entry.restored_entity_count !== null && `恢复 ${entry.restored_entity_count} 处`}
                                {entry.error_code && `　错误码: ${entry.error_code}`}
                              </p>
                              {entry.batch_id && (
                                <p className="text-xs text-gray-400 mt-0.5 font-mono truncate" title={entry.batch_id}>
                                  批次: {entry.batch_id}
                                </p>
                              )}
                            </div>
                            <span className="text-xs text-gray-400 shrink-0">
                              {formatDate(new Date(entry.timestamp).getTime())}
                            </span>
                          </div>
                        ))
                      )}
                    </div>

                    {totalPages > 1 && (
                      <div className="flex justify-center pt-4 border-t">
                        <Pagination>
                          <PaginationContent>
                            <PaginationItem>
                              <PaginationPrevious
                                onClick={() => handlePageChange(currentPage - 1)}
                                className={cn(currentPage <= 1 && "pointer-events-none opacity-50")}
                              />
                            </PaginationItem>
                            <PaginationItem>
                              <span className="px-3 text-sm text-gray-600">
                                第 {currentPage} / {totalPages} 页
                              </span>
                            </PaginationItem>
                            <PaginationItem>
                              <PaginationNext
                                onClick={() => handlePageChange(currentPage + 1)}
                                className={cn(currentPage >= totalPages && "pointer-events-none opacity-50")}
                              />
                            </PaginationItem>
                          </PaginationContent>
                        </Pagination>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            </>
          )}
        </div>
      </div>
      <ConfirmDialog
        open={confirmClearOpen}
        title="确认清空操作日志"
        description="这会删除当前页面里的操作记录，但不会删除任何批次、文件或处理结果。清空后无法恢复。"
        confirmLabel="确认清空"
        confirming={clearing}
        onConfirm={() => void handleClear()}
        onOpenChange={setConfirmClearOpen}
      />
    </div>
  );
}
