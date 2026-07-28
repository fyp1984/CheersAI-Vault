import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronRight, Plus, Search, X } from "lucide-react";
import { Link } from "react-router-dom";
import { api } from "../api/client";
import { EmptyState, ErrorState, LoadingState } from "../components/Feedback";
import { StatusBadge } from "../components/StatusBadge";
import type { BatchStatus, BatchSummary } from "../types";

function formatTime(value: string): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit",
  }).format(new Date(value));
}

const STATUS_OPTIONS: Array<{ value: BatchStatus | ""; label: string }> = [
  { value: "", label: "全部状态" },
  { value: "Running", label: "处理中" },
  { value: "Completed", label: "已完成" },
  { value: "CompletedWithErrors", label: "部分失败" },
  { value: "Failed", label: "失败" },
];

function matchesSearch(batch: BatchSummary, query: string): boolean {
  if (!query.trim()) return true;
  const q = query.trim().toLowerCase();
  return batch.batch_id.toLowerCase().includes(q);
}

function matchesStatus(batch: BatchSummary, filter: BatchStatus | ""): boolean {
  if (!filter) return true;
  return batch.status === filter;
}

export function BatchesPage() {
  const [batches, setBatches] = useState<BatchSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<BatchStatus | "">("");

  const load = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    setError(null);
    try {
      const response = await api.batches();
      setBatches(response.batches);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "处理日志加载失败。");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (!batches.some((batch) => batch.status === "Running")) return;
    const timer = window.setTimeout(() => void load(true), 1500);
    return () => window.clearTimeout(timer);
  }, [batches, load]);

  const filtered = useMemo(() => {
    return batches.filter((b) => matchesSearch(b, searchQuery) && matchesStatus(b, statusFilter));
  }, [batches, searchQuery, statusFilter]);

  const hasFilters = searchQuery.trim() !== "" || statusFilter !== "";
  const totalMasked = useMemo(
    () => filtered.reduce((sum, b) => sum + b.masked_entity_count, 0),
    [filtered],
  );

  function clearFilters() {
    setSearchQuery("");
    setStatusFilter("");
  }

  return (
    <section className="page">
      <div className="page-heading compact">
        <div><h1>处理日志</h1><p>所有状态来自 Runtime 持久化记录，重启后可继续读取。</p></div>
        <Link className="button primary" to="/submit"><Plus aria-hidden="true" />新建批次</Link>
      </div>
      <div className="toolbar filters-bar">
        <div className="search-wrap">
          <Search className="search-icon" aria-hidden="true" size={16} />
          <input
            className="search-input"
            type="text"
            placeholder="按批次 ID 搜索…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            aria-label="搜索批次 ID"
          />
          {searchQuery && (
            <button className="button-link clear-btn" type="button" onClick={() => setSearchQuery("")} aria-label="清除搜索">
              <X size={14} />
            </button>
          )}
        </div>
        <select
          className="status-select"
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as BatchStatus | "")}
          aria-label="按状态筛选"
        >
          {STATUS_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>{opt.label}</option>
          ))}
        </select>
        {hasFilters && (
          <button className="button secondary" type="button" onClick={clearFilters}>
            清空条件
          </button>
        )}
      </div>
      {loading ? (
        <LoadingState label="正在读取处理日志…" />
      ) : error ? (
        <ErrorState message={error} onRetry={() => void load()} />
      ) : batches.length === 0 ? (
        <EmptyState title="还没有批次" detail="前往批量提交入口创建第一个真实脱敏作业。" />
      ) : filtered.length === 0 ? (
        <div className="state-panel">
          <div>
            <strong>没有匹配的记录</strong>
            <p>当前筛选条件没有匹配的处理日志，试试调整搜索关键词或状态筛选。</p>
            <button className="button secondary" type="button" onClick={clearFilters} style={{ marginTop: 12 }}>
              清空筛选条件
            </button>
          </div>
        </div>
      ) : (
        <div className="panel table-panel">
          <table>
            <thead>
              <tr>
                <th>批次</th>
                <th>状态</th>
                <th>文件</th>
                <th>成功</th>
                <th>失败</th>
                <th>脱敏总数</th>
                <th>更新时间</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {filtered.map((batch) => (
                <tr key={batch.batch_id}>
                  <td><span className="mono id-cell">{batch.batch_id.slice(0, 8)}</span></td>
                  <td><StatusBadge status={batch.status} /></td>
                  <td>{batch.file_count}</td>
                  <td>{batch.completed_count}</td>
                  <td>{batch.failed_count}</td>
                  <td>{batch.masked_entity_count > 0 ? batch.masked_entity_count.toLocaleString() : "—"}</td>
                  <td className="muted">{formatTime(batch.updated_at)}</td>
                  <td>
                    <Link className="text-link" to={`/batches/${batch.batch_id}`}>
                      查看详情<ChevronRight aria-hidden="true" />
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {hasFilters && (
            <div className="filter-summary">
              {filtered.length} 条记录，脱敏总数 {totalMasked.toLocaleString()}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
