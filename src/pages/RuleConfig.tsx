import { useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Trash2, Plus, ChevronUp } from "lucide-react";
import { useRuleStore } from "@/store/ruleStore";
import { v4 as uuidv4 } from "uuid";

export default function RuleConfig() {
  const { rules, toggleRule, addRule, removeRule } = useRuleStore();

  const builtin = rules.filter((r) => r.builtin);
  const custom = rules.filter((r) => !r.builtin);

  const [showAddForm, setShowAddForm] = useState(false);
  const [form, setForm] = useState({ name: "", pattern: "", replacement_template: "", use_counter: false });
  const [patternError, setPatternError] = useState("");

  const validatePattern = (p: string) => {
    try { new RegExp(p); return ""; }
    catch (e) { return `正则表达式无效: ${(e as Error).message}`; }
  };

  const handleAdd = () => {
    const err = validatePattern(form.pattern);
    if (err) { setPatternError(err); return; }
    if (!form.name.trim() || !form.pattern.trim() || !form.replacement_template.trim()) {
      setPatternError("请填写完整的规则信息"); return;
    }
    addRule({
      id: `custom_${uuidv4()}`,
      name: form.name.trim(),
      pattern: form.pattern.trim(),
      replacement: form.replacement_template.trim(),
      replacement_template: form.replacement_template.trim(),
      use_counter: form.use_counter,
      enabled: true,
      builtin: false,
      description: `自定义规则：${form.pattern.trim()}`,
    });
    setForm({ name: "", pattern: "", replacement_template: "", use_counter: false });
    setPatternError("");
    setShowAddForm(false);
  };

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="规则配置" description="管理内置与自定义脱敏规则" />
      <div className="flex-1 overflow-auto p-6 space-y-6">

        {/* 内置规则 */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">内置规则</CardTitle>
          </CardHeader>
          <CardContent className="space-y-1">
            {builtin.map((rule) => (
              <div key={rule.id} className="flex items-center justify-between py-2 border-b border-gray-100 last:border-0">
                <div>
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-gray-800">{rule.name}</span>
                    <Badge variant="outline" className="text-xs">内置</Badge>
                  </div>
                  {rule.description && (
                    <p className="text-xs text-gray-400 mt-0.5">{rule.description}</p>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  <Label htmlFor={`rule-${rule.id}`} className="text-xs text-gray-500">
                    {rule.enabled ? "已启用" : "已禁用"}
                  </Label>
                  <Switch
                    id={`rule-${rule.id}`}
                    checked={rule.enabled}
                    onCheckedChange={() => toggleRule(rule.id)}
                  />
                </div>
              </div>
            ))}
          </CardContent>
        </Card>

        {/* 自定义规则 */}
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle className="text-base">自定义规则</CardTitle>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setShowAddForm((v) => !v)}
              className="flex items-center gap-1 text-xs"
            >
              {showAddForm ? <ChevronUp className="w-3 h-3" /> : <Plus className="w-3 h-3" />}
              {showAddForm ? "收起" : "新增规则"}
            </Button>
          </CardHeader>
          <CardContent className="space-y-3">

            {/* 新增表单 */}
            {showAddForm && (
              <div className="border border-blue-200 rounded-lg p-4 bg-blue-50 space-y-3">
                <p className="text-xs font-medium text-blue-900">自定义正则规则</p>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1">
                    <Label className="text-xs">规则名称</Label>
                    <Input
                      placeholder="例：项目代号"
                      value={form.name}
                      onChange={(e) => setForm({ ...form, name: e.target.value })}
                      className="text-sm h-8"
                    />
                  </div>
                  <div className="space-y-1">
                    <Label className="text-xs">替换为</Label>
                    <Input
                      placeholder="例：***PROJECT***"
                      value={form.replacement_template}
                      onChange={(e) => setForm({ ...form, replacement_template: e.target.value })}
                      className="text-sm h-8 font-mono"
                    />
                  </div>
                </div>
                <div className="space-y-1">
                  <Label className="text-xs">正则表达式（Regex）</Label>
                  <Input
                    placeholder="例：Project[A-Z]{2}\d{4} 或 \\b张三\\b"
                    value={form.pattern}
                    onChange={(e) => { setForm({ ...form, pattern: e.target.value }); setPatternError(""); }}
                    className={`text-sm h-8 font-mono ${patternError ? "border-red-400" : ""}`}
                  />
                  {patternError && <p className="text-xs text-red-600">{patternError}</p>}
                  <p className="text-xs text-gray-400">
                    支持标准正则。例：<code className="bg-gray-100 px-1">王明|张华|李刚</code> 可替换特定姓名；
                    <code className="bg-gray-100 px-1">CheersAI|公司A</code> 可替换公司名。
                  </p>
                </div>
                <div className="flex items-center justify-between pt-1">
                  <div className="flex items-center gap-2">
                    <Switch
                      id="use-counter"
                      checked={form.use_counter}
                      onCheckedChange={(v) => setForm({ ...form, use_counter: v })}
                    />
                    <Label htmlFor="use-counter" className="text-xs cursor-pointer">
                      {form.use_counter
                        ? "序号替换（每次匹配追加 1、2、3…区分不同实例）"
                        : "固定文本（所有匹配统一替换为相同文本）"}
                    </Label>
                  </div>
                </div>
                <div className="flex gap-2">
                  <Button size="sm" onClick={handleAdd} className="text-xs">保存规则</Button>
                  <Button size="sm" variant="outline" onClick={() => { setShowAddForm(false); setPatternError(""); }} className="text-xs">取消</Button>
                </div>
              </div>
            )}

            {/* 已有自定义规则列表 */}
            {custom.length === 0 && !showAddForm && (
              <p className="text-sm text-gray-400 py-4 text-center">
                暂无自定义规则。点击「新增规则」创建基于正则的脱敏规则。
              </p>
            )}
            {custom.map((rule) => (
              <div key={rule.id} className="flex items-center justify-between py-2 border-b border-gray-100 last:border-0">
                <div className="min-w-0 flex-1 mr-3">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-gray-800">{rule.name}</span>
                    <Badge variant="secondary" className="text-xs">自定义</Badge>
                  </div>
                  <p className="text-xs text-gray-400 font-mono mt-0.5 truncate">
                    {rule.replacement_template} ← /{rule.pattern}/
                  </p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <Switch
                    checked={rule.enabled}
                    onCheckedChange={() => toggleRule(rule.id)}
                  />
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => removeRule(rule.id)}
                    className="text-red-500 hover:text-red-700 hover:bg-red-50 p-1 h-7 w-7"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </Button>
                </div>
              </div>
            ))}
          </CardContent>
        </Card>

        {/* 使用说明 */}
        <Card className="border-amber-200 bg-amber-50">
          <CardContent className="pt-4">
            <p className="text-xs font-medium text-amber-800 mb-2">💡 使用提示</p>
            <ul className="text-xs text-amber-700 space-y-1 list-disc list-inside">
              <li>内置规则用于识别手机号、身份证等标准 PII 格式</li>
              <li>自定义规则可替换项目代号、公司名称、特定人名等业务敏感词</li>
              <li>在文件处理页选择规则后，自定义规则会自动生效</li>
              <li>替换文本末尾加数字序号可区分不同实例，如 <code className="bg-amber-100 px-1">姓名</code> → 姓名1、姓名2…</li>
            </ul>
          </CardContent>
        </Card>

      </div>
    </div>
  );
}
