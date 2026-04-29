import { useState, useEffect } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Plus, Trash2, Search, Download, Upload, ChevronUp } from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import type { SensitiveTerm, AddSensitiveTermRequest } from "@/types/commands";
import { open, save } from "@tauri-apps/plugin-dialog";
import Toast from "@/components/common/Toast";

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
  const [form, setForm] = useState<AddSensitiveTermRequest>({ term: "", category: "", description: "" });
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<ToastMessage | null>(null);

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

  const handleAdd = async () => {
    if (!form.term.trim() || !form.category.trim()) {
      setToast({ message: "请填写敏感词和分类", type: "error" });
      return;
    }

    try {
      setLoading(true);
      await tauriCommands.addSensitiveTerm(form);
      setToast({ message: "添加成功", type: "success" });
      setForm({ term: "", category: "", description: "" });
      setShowAddForm(false);
      await loadData();
    } catch (error) {
      console.error("Failed to add term:", error);
      setToast({ message: "添加失败", type: "error" });
    } finally {
      setLoading(false);
    }
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
      setToast({ message: "导入失败", type: "error" });
    }
  };

  const filteredTerms = searchQuery
    ? terms
    : terms.filter((t) => !selectedCategory || t.category === selectedCategory);

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
              <div className="text-2xl font-bold text-green-600">{stats.enabled}</div>
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
          <CardContent className="pt-4">
            <div className="flex items-center gap-3">
              <div className="flex-1 flex items-center gap-2">
                <Input
                  placeholder="搜索敏感词..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                  className="max-w-xs"
                />
                <Button size="sm" variant="outline" onClick={handleSearch}>
                  <Search className="w-4 h-4" />
                </Button>
              </div>
              
              <select
                value={selectedCategory}
                onChange={(e) => setSelectedCategory(e.target.value)}
                className="px-3 py-1.5 text-sm border rounded-md"
              >
                <option value="">全部分类</option>
                {categories.map((cat) => (
                  <option key={cat} value={cat}>{cat}</option>
                ))}
              </select>

              <Button size="sm" variant="outline" onClick={handleExport}>
                <Download className="w-4 h-4 mr-1" />
                导出
              </Button>
              <Button size="sm" variant="outline" onClick={handleImport}>
                <Upload className="w-4 h-4 mr-1" />
                导入
              </Button>
              <Button size="sm" onClick={() => setShowAddForm(!showAddForm)}>
                {showAddForm ? <ChevronUp className="w-4 h-4 mr-1" /> : <Plus className="w-4 h-4 mr-1" />}
                {showAddForm ? "收起" : "添加"}
              </Button>
            </div>
          </CardContent>
        </Card>

        {/* 添加表单 */}
        {showAddForm && (
          <Card className="border-blue-200 bg-blue-50">
            <CardContent className="pt-4 space-y-3">
              <p className="text-sm font-medium text-blue-900">添加敏感词</p>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1">
                  <Label className="text-xs">敏感词</Label>
                  <Input
                    placeholder="例：张三"
                    value={form.term}
                    onChange={(e) => setForm({ ...form, term: e.target.value })}
                    className="text-sm h-8"
                  />
                </div>
                <div className="space-y-1">
                  <Label className="text-xs">分类</Label>
                  <Input
                    placeholder="例：姓名"
                    value={form.category}
                    onChange={(e) => setForm({ ...form, category: e.target.value })}
                    className="text-sm h-8"
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
                  className="text-sm h-8"
                />
              </div>
              <div className="flex gap-2">
                <Button size="sm" onClick={handleAdd} disabled={loading}>
                  保存
                </Button>
                <Button size="sm" variant="outline" onClick={() => setShowAddForm(false)}>
                  取消
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {/* 词条列表 */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">
              敏感词列表 ({filteredTerms.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            {filteredTerms.length === 0 ? (
              <p className="text-sm text-gray-400 py-8 text-center">
                暂无敏感词。点击「添加」按钮创建敏感词条。
              </p>
            ) : (
              <div className="space-y-1">
                {filteredTerms.map((term) => (
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
          </CardContent>
        </Card>

        {/* 使用说明 */}
        <Card className="border-amber-200 bg-amber-50">
          <CardContent className="pt-4">
            <p className="text-xs font-medium text-amber-800 mb-2">💡 使用提示</p>
            <ul className="text-xs text-amber-700 space-y-1 list-disc list-inside">
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
