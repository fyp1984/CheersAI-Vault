import { useState, useEffect, useMemo } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Plus, Trash2, Search, Download, Upload, ChevronUp, Lightbulb, FileSpreadsheet, Pencil } from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import type { SensitiveTerm, AddSensitiveTermRequest } from "@/types/commands";
import { open, save } from "@tauri-apps/plugin-dialog";
import Toast from "@/components/common/Toast";
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ToastMessage {
  message: string;
  type: "success" | "error" | "info";
}

export default function SensitiveTerms() {
  const [terms, setTerms] = useState<SensitiveTerm[]>([]);
  const [categories, setCategories] = useState<string[]>([]);
  const [selectedCategory, setSelectedCategory] = useState<string>("");
  const [searchQuery, setSearchQuery] = useState("");
  const [stats, setStats] = useState({ total: 0, enabled: 0, disabled: 0, categories: 0 });
  const [showAddForm, setShowAddForm] = useState(false);
  const [editingTerm, setEditingTerm] = useState<SensitiveTerm | null>(null);
  const [form, setForm] = useState<AddSensitiveTermRequest>({ term: "", category: "", description: "" });
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  
  // 分页和排序状态
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(5);
  const [sortBy, setSortBy] = useState<'time' | 'alpha'>('time'); // time: 时间新旧, alpha: 首字母

  useEffect(() => {
    loadData();
  }, [selectedCategory]);

  const loadData = async () => {
    try {
      const [termsData, categoriesData, statsData] = await Promise.all([
        tauriCommands.getSensitiveTerms(selectedCategory || undefined, false),
        tauriCommands.getSensitiveTermCategories(),
        tauriCommands.getSensitiveTermsStats(),
      ]);
      setTerms(termsData);
      setCategories(categoriesData);
      setStats(statsData);
    } catch (error) {
      console.error("Failed to load data:", error);
      setToast({ message: "加载数据失败", type: "error" });
    }
  };

  const handleSaveTerm = async () => {
    if (!form.term.trim() || !form.category.trim()) {
      setToast({ message: "请填写敏感词和分类", type: "error" });
      return;
    }

    try {
      setLoading(true);
      if (editingTerm) {
        await tauriCommands.updateSensitiveTerm({
          id: editingTerm.id,
          term: form.term,
          category: form.category,
          description: form.description,
        });
        setToast({ message: "修改成功", type: "success" });
      } else {
        await tauriCommands.addSensitiveTerm(form);
        setToast({ message: "添加成功", type: "success" });
      }
      setForm({ term: "", category: "", description: "" });
      setEditingTerm(null);
      setShowAddForm(false);
      await loadData();
    } catch (error) {
      console.error("Failed to save term:", error);
      const message = error instanceof Error ? error.message : String(error);
      setToast({ message: message || (editingTerm ? "修改失败" : "添加失败"), type: "error" });
    } finally {
      setLoading(false);
    }
  };

  const handleEdit = (term: SensitiveTerm) => {
    setEditingTerm(term);
    setForm({
      term: term.term,
      category: term.category,
      description: term.description || "",
    });
    setShowAddForm(true);
  };

  const handleCancelEdit = () => {
    setEditingTerm(null);
    setForm({ term: "", category: "", description: "" });
    setShowAddForm(false);
  };

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await tauriCommands.updateSensitiveTerm({ id, enabled });
      await loadData();
    } catch (error) {
      console.error("Failed to toggle term:", error);
      setToast({ message: "更新失败", type: "error" });
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await tauriCommands.deleteSensitiveTerm(id);
      setToast({ message: "删除成功", type: "success" });
      await loadData();
    } catch (error) {
      console.error("Failed to delete term:", error);
      setToast({ message: "删除失败", type: "error" });
    }
  };

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      await loadData();
      return;
    }

    try {
      const results = await tauriCommands.searchSensitiveTerms(searchQuery);
      setTerms(results);
    } catch (error) {
      console.error("Failed to search:", error);
      setToast({ message: "搜索失败", type: "error" });
    }
  };

  const handleExport = async () => {
    try {
      const filePath = await save({
        defaultPath: "sensitive_terms.csv",
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });

      if (filePath) {
        await tauriCommands.exportSensitiveTermsCsv(filePath);
        setToast({ message: "导出成功", type: "success" });
      }
    } catch (error) {
      console.error("Failed to export:", error);
      setToast({ message: "导出失败", type: "error" });
    }
  };

  const handleImport = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });

      if (selected) {
        const count = await tauriCommands.importSensitiveTermsCsv(selected as string);
        setToast({ message: `成功导入 ${count} 条记录`, type: "success" });
        await loadData();
      }
    } catch (error) {
      console.error("Failed to import:", error);
      const message = error instanceof Error ? error.message : String(error);
      setToast({ message: message || "导入失败", type: "error" });
    }
  };

  const filteredTerms = searchQuery
    ? terms
    : terms.filter((t) => !selectedCategory || t.category === selectedCategory);

  // 排序和分页逻辑
  const sortedAndPagedTerms = useMemo(() => {
    // 1. 排序
    let sorted = [...filteredTerms];
    if (sortBy === 'time') {
      // 按时间排序（新的在前）
      sorted.sort((a, b) => {
        const dateA = new Date(a.created_at || 0).getTime();
        const dateB = new Date(b.created_at || 0).getTime();
        return dateB - dateA;
      });
    } else {
      // 按首字母排序
      sorted.sort((a, b) => {
        return a.term.localeCompare(b.term, 'zh-CN');
      });
    }
    
    // 2. 分页
    const startIndex = (currentPage - 1) * pageSize;
    const endIndex = startIndex + pageSize;
    return sorted.slice(startIndex, endIndex);
  }, [filteredTerms, sortBy, currentPage, pageSize]);

  // 总页数
  const totalPages = Math.ceil(filteredTerms.length / pageSize);
  const selectTriggerClass =
    "h-10 rounded-xl border-gray-200 bg-white px-4 text-sm text-gray-700 shadow-sm transition-all hover:border-blue-200 hover:bg-blue-50/30 focus:ring-2 focus:ring-blue-100 focus:ring-offset-0 data-[state=open]:border-blue-300 data-[state=open]:bg-blue-50/40";
  const selectContentClass =
    "rounded-xl border-gray-200 bg-white p-1.5 shadow-xl shadow-slate-900/10";
  const selectItemClass =
    "cursor-pointer rounded-lg py-2 pl-8 pr-3 text-sm text-gray-700 focus:bg-blue-50 focus:text-blue-700 data-[state=checked]:bg-blue-50 data-[state=checked]:font-medium data-[state=checked]:text-blue-700";

  // 当筛选条件变化时，重置到第一页
  useEffect(() => {
    setCurrentPage(1);
  }, [selectedCategory, searchQuery, pageSize]);

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="敏感词库" description="管理脱敏时需要匹配的敏感信息" />
      
      <div className="flex-1 overflow-auto p-6 space-y-6">
        {/* 统计信息 */}
        <div className="grid grid-cols-4 gap-4">
          <Card>
            <CardContent className="pt-4">
              <div className="text-2xl font-bold">{stats.total}</div>
              <p className="text-xs text-gray-500">总词条数</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4">
              <div className="text-2xl font-bold text-blue-600">{stats.enabled}</div>
              <p className="text-xs text-gray-500">已启用</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4">
              <div className="text-2xl font-bold text-gray-400">{stats.disabled}</div>
              <p className="text-xs text-gray-500">已禁用</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4">
              <div className="text-2xl font-bold text-blue-600">{stats.categories}</div>
              <p className="text-xs text-gray-500">分类数</p>
            </CardContent>
          </Card>
        </div>

        {/* 操作栏 */}
        <Card>
          <CardContent className="py-4">
            <div className="flex flex-wrap items-center gap-3">
              <div className="flex min-w-[260px] flex-1 items-center gap-2 text-sm text-gray-500">
                <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-blue-50 text-blue-600">
                  <FileSpreadsheet className="h-4 w-4" />
                </div>
                <div>
                  <p className="font-medium text-gray-700">批量维护敏感词库</p>
                  <p className="text-xs text-gray-400">支持 CSV 导入导出，便于备份和跨设备迁移</p>
                </div>
              </div>
              <Button size="sm" variant="outline" onClick={handleExport} className="h-9">
                <Download className="w-4 h-4 mr-1" />
                导出
              </Button>
              <Button size="sm" variant="outline" onClick={handleImport} className="h-9">
                <Upload className="w-4 h-4 mr-1" />
                导入
              </Button>
              <Button
                size="sm"
                onClick={() => {
                  if (showAddForm) {
                    handleCancelEdit();
                  } else {
                    setEditingTerm(null);
                    setForm({ term: "", category: "", description: "" });
                    setShowAddForm(true);
                  }
                }}
                className="h-9"
              >
                {showAddForm ? <ChevronUp className="w-4 h-4 mr-1" /> : <Plus className="w-4 h-4 mr-1" />}
                {showAddForm ? "收起" : "添加"}
              </Button>
            </div>
          </CardContent>
        </Card>

        {/* 添加表单 */}
        {showAddForm && (
          <Card className="border-blue-200 bg-blue-50/70">
            <CardContent className="space-y-3 p-4">
              <p className="text-sm font-medium text-blue-900">
                {editingTerm ? "修改敏感词" : "添加敏感词"}
              </p>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                <div className="space-y-1">
                  <Label className="text-xs">敏感词</Label>
                  <Input
                    placeholder="例：张三"
                    value={form.term}
                    onChange={(e) => setForm({ ...form, term: e.target.value })}
                    className="h-9 bg-white text-sm"
                  />
                </div>
                <div className="space-y-1">
                  <Label className="text-xs">分类</Label>
                  <Input
                    placeholder="例：姓名"
                    value={form.category}
                    onChange={(e) => setForm({ ...form, category: e.target.value })}
                    className="h-9 bg-white text-sm"
                    list="categories-list"
                  />
                  <datalist id="categories-list">
                    {categories.map((cat) => (
                      <option key={cat} value={cat} />
                    ))}
                  </datalist>
                </div>
              </div>
              <div className="space-y-1">
                <Label className="text-xs">描述（可选）</Label>
                <Input
                  placeholder="例：测试人员姓名"
                  value={form.description}
                  onChange={(e) => setForm({ ...form, description: e.target.value })}
                  className="h-9 bg-white text-sm"
                />
              </div>
              <div className="flex gap-2">
                <Button size="sm" onClick={handleSaveTerm} disabled={loading}>
                  {editingTerm ? "保存修改" : "保存"}
                </Button>
                <Button size="sm" variant="outline" onClick={handleCancelEdit}>
                  取消
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {/* 词条列表 */}
        <Card>
          <CardHeader className="space-y-4 pb-4">
            <div className="flex items-center justify-between gap-4">
              <CardTitle className="text-base">
                敏感词列表 (共 {filteredTerms.length} 条)
              </CardTitle>
              <div className="shrink-0 text-sm text-gray-500">
                第 {filteredTerms.length > 0 ? (currentPage - 1) * pageSize + 1 : 0} - {Math.min(currentPage * pageSize, filteredTerms.length)} 条
              </div>
            </div>

            <div className="rounded-xl border border-gray-100 bg-gray-50/70 p-3">
              <div className="flex flex-wrap items-center gap-2.5">
                <div className="flex min-w-[260px] flex-1 items-center gap-2">
                  <Input
                    placeholder="搜索敏感词..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                    className="h-9 bg-white"
                  />
                  <Button size="sm" variant="outline" onClick={handleSearch} className="h-9 px-3">
                    <Search className="w-4 h-4" />
                  </Button>
                </div>

                <Select value={selectedCategory || "__all__"} onValueChange={(value) => setSelectedCategory(value === "__all__" ? "" : value)}>
                  <SelectTrigger className={`${selectTriggerClass} w-[144px]`}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent className={selectContentClass}>
                    <SelectItem value="__all__" className={selectItemClass}>全部分类</SelectItem>
                    {categories.map((cat) => (
                      <SelectItem key={cat} value={cat} className={selectItemClass}>{cat}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <Select value={sortBy} onValueChange={(value: 'time' | 'alpha') => setSortBy(value)}>
                  <SelectTrigger className={`${selectTriggerClass} w-[144px]`}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent className={selectContentClass}>
                    <SelectItem value="time" className={selectItemClass}>时间排序</SelectItem>
                    <SelectItem value="alpha" className={selectItemClass}>首字母排序</SelectItem>
                  </SelectContent>
                </Select>

                <Select value={pageSize.toString()} onValueChange={(value) => setPageSize(Number(value))}>
                  <SelectTrigger className={`${selectTriggerClass} w-[132px]`}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent className={selectContentClass}>
                    <SelectItem value="5" className={selectItemClass}>每页 5 条</SelectItem>
                    <SelectItem value="10" className={selectItemClass}>每页 10 条</SelectItem>
                    <SelectItem value="20" className={selectItemClass}>每页 20 条</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </CardHeader>

          <CardContent className="pt-0">
            {sortedAndPagedTerms.length === 0 ? (
              <p className="text-sm text-gray-400 py-8 text-center">
                {filteredTerms.length === 0 
                  ? "暂无敏感词。点击「添加」按钮创建敏感词条。"
                  : "当前页无数据"}
              </p>
            ) : (
              <div className="space-y-1">
                {sortedAndPagedTerms.map((term) => (
                  <div
                    key={term.id}
                    className="flex items-center justify-between py-2 border-b border-gray-100 last:border-0"
                  >
                    <div className="flex-1 min-w-0 mr-3">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-gray-800">{term.term}</span>
                        <Badge variant="outline" className="text-xs">{term.category}</Badge>
                        {!term.enabled && (
                          <Badge variant="secondary" className="text-xs">已禁用</Badge>
                        )}
                      </div>
                      {term.description && (
                        <p className="text-xs text-gray-400 mt-0.5">{term.description}</p>
                      )}
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleEdit(term)}
                        className="h-7 px-2 text-blue-600 hover:bg-blue-50 hover:text-blue-700"
                      >
                        <Pencil className="w-3.5 h-3.5 mr-1" />
                        修改
                      </Button>
                      <Switch
                        checked={term.enabled}
                        onCheckedChange={(checked) => handleToggle(term.id, checked)}
                      />
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleDelete(term.id)}
                        className="text-red-500 hover:text-red-700 hover:bg-red-50 p-1 h-7 w-7"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
            
            {/* 分页组件 */}
            {totalPages > 1 && (
              <div className="mt-4 flex items-center justify-center">
                <Pagination>
                  <PaginationContent>
                    <PaginationItem>
                      <PaginationPrevious 
                        onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                        className={currentPage === 1 ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                      />
                    </PaginationItem>
                    
                    {Array.from({ length: totalPages }, (_, i) => i + 1).map((page) => {
                      // 只显示当前页附近的页码
                      if (
                        page === 1 ||
                        page === totalPages ||
                        (page >= currentPage - 1 && page <= currentPage + 1)
                      ) {
                        return (
                          <PaginationItem key={page}>
                            <PaginationLink
                              onClick={() => setCurrentPage(page)}
                              isActive={currentPage === page}
                              className="cursor-pointer"
                            >
                              {page}
                            </PaginationLink>
                          </PaginationItem>
                        );
                      } else if (page === currentPage - 2 || page === currentPage + 2) {
                        return (
                          <PaginationItem key={page}>
                            <span className="px-2">...</span>
                          </PaginationItem>
                        );
                      }
                      return null;
                    })}
                    
                    <PaginationItem>
                      <PaginationNext 
                        onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                        className={currentPage === totalPages ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                      />
                    </PaginationItem>
                  </PaginationContent>
                </Pagination>
              </div>
            )}
          </CardContent>
        </Card>

        {/* 使用说明 */}
        <Card className="border-blue-200 bg-blue-50">
          <CardContent className="pt-4">
            <p className="text-xs font-medium text-blue-800 mb-2 flex items-center gap-1.5">
              <Lightbulb className="w-4 h-4" />
              使用提示
            </p>
            <ul className="text-xs text-blue-700 space-y-1 list-disc list-inside">
              <li>敏感词会在脱敏时进行精确匹配和替换</li>
              <li>可以按分类组织敏感词，如：姓名、地址、公司名等</li>
              <li>支持CSV批量导入导出，格式：分类,敏感词,描述,状态</li>
              <li>禁用的敏感词不会参与脱敏匹配</li>
              <li>建议定期导出备份敏感词库</li>
            </ul>
          </CardContent>
        </Card>
      </div>

      {/* Toast */}
      {toast && (
        <Toast
          message={toast.message}
          type={toast.type}
          onClose={() => setToast(null)}
        />
      )}
    </div>
  );
}
