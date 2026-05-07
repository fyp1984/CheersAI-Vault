import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Lightbulb } from "lucide-react";
import { useRuleStore } from "@/store/ruleStore";
import { tauriCommands } from "@/lib/tauri";
import { useNavigate } from "react-router-dom";

interface RuleSelectorProps {
  selectedRules: string[];
  onRulesChange: (ruleIds: string[]) => void;
}

const saveRules = (ruleIds: string[]) => {
  try { localStorage.setItem("selected-rules", JSON.stringify(ruleIds)); }
  catch { /* ignore */ }
};

export function RuleSelector({ selectedRules, onRulesChange }: RuleSelectorProps) {
  const { rules } = useRuleStore();
  const navigate = useNavigate();
  const [sensitiveTermsCount, setSensitiveTermsCount] = useState<number | null>(null);

  // 获取敏感词库统计
  useEffect(() => {
    const fetchStats = async () => {
      try {
        console.log("Fetching sensitive terms stats...");
        const stats = await tauriCommands.getSensitiveTermsStats();
        console.log("Sensitive terms stats received:", stats);
        console.log("Stats type:", typeof stats);
        console.log("Stats.enabled:", stats.enabled);
        console.log("Stats.enabled type:", typeof stats.enabled);
        setSensitiveTermsCount(stats.enabled);
      } catch (error) {
        console.error("Failed to fetch sensitive terms stats:", error);
        setSensitiveTermsCount(0);
      }
    };
    fetchStats();
  }, []);

  // 初始化时恢复 localStorage 选择或使用默认启用规则
  useEffect(() => {
    // 只在规则加载完成且还没有选择时初始化
    if (rules.length === 0) return;
    if (selectedRules.length > 0) return;
    
    try {
      const saved = localStorage.getItem("selected-rules");
      const savedIds: string[] = saved ? JSON.parse(saved) : [];
      // 过滤掉不存在的规则ID
      const valid = savedIds.filter((id) => 
        rules.some((r) => r.id === id)
      );
      if (valid.length > 0) { 
        console.log("Restoring saved rules:", valid);
        // 始终添加 use_sensitive_terms
        const withSensitiveTerms = valid.includes("use_sensitive_terms") 
          ? valid 
          : [...valid, "use_sensitive_terms"];
        onRulesChange(withSensitiveTerms); 
        return; 
      }
    } catch { /* ignore */ }
    
    // 默认选择所有启用的规则 + 敏感词库
    const defaultRules = rules.filter((r) => r.enabled).map((r) => r.id);
    const defaultWithSensitiveTerms = [...defaultRules, "use_sensitive_terms"];
    console.log("Using default enabled rules:", defaultWithSensitiveTerms);
    onRulesChange(defaultWithSensitiveTerms);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rules]);

  // 确保 use_sensitive_terms 始终在 selectedRules 中
  useEffect(() => {
    if (selectedRules.length > 0 && !selectedRules.includes("use_sensitive_terms")) {
      console.log("Adding use_sensitive_terms to selectedRules");
      onRulesChange([...selectedRules, "use_sensitive_terms"]);
    }
  }, [selectedRules, onRulesChange]);

  const handleToggle = (ruleId: string, checked: boolean) => {
    const next = checked
      ? [...selectedRules, ruleId]
      : selectedRules.filter((id) => id !== ruleId);
    // 确保 use_sensitive_terms 始终存在
    const withSensitiveTerms = next.includes("use_sensitive_terms") 
      ? next 
      : [...next, "use_sensitive_terms"];
    onRulesChange(withSensitiveTerms);
    saveRules(withSensitiveTerms);
  };

  const builtin = rules.filter((r) => r.builtin);
  const custom = rules.filter((r) => !r.builtin);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-sm">脱敏规则</CardTitle>
        <p className="text-xs text-gray-500">选择后自动保存</p>
      </CardHeader>
      <CardContent className="space-y-2">
        {builtin.map((rule) => (
          <div key={rule.id} className="flex items-center justify-between">
            <Label htmlFor={rule.id} className="text-sm font-normal cursor-pointer">
              {rule.name}
            </Label>
            <Switch
              id={rule.id}
              checked={selectedRules.includes(rule.id)}
              onCheckedChange={(checked) => handleToggle(rule.id, checked)}
            />
          </div>
        ))}
        
        {/* 敏感词库 - 始终启用，不显示开关 */}
        <div className="border-t pt-2 mt-2">
          {sensitiveTermsCount === null ? (
            // 加载中
            <div className="flex items-center justify-between mb-1 bg-gray-50 px-2 py-1.5 rounded">
              <Label className="text-sm font-normal text-gray-600">
                敏感词库（加载中...）
              </Label>
            </div>
          ) : sensitiveTermsCount === 0 ? (
            // 未配置敏感词库 - 显示提示
            <div className="bg-amber-50 border border-amber-200 px-3 py-2.5 rounded-lg">
              <div className="flex items-start gap-2 mb-2">
                <Lightbulb className="w-4 h-4 text-amber-600 flex-shrink-0 mt-0.5" />
                <div className="flex-1">
                  <p className="text-sm font-medium text-amber-900 mb-1">
                    为了效果更好更有针对性，请配置您专属的敏感词库
                  </p>
                  <p className="text-xs text-amber-700">
                    添加您业务相关的敏感词，如公司名称、项目代号、内部术语等
                  </p>
                </div>
              </div>
              <button
                onClick={() => navigate("/sensitive-terms")}
                className="w-full text-xs bg-amber-600 hover:bg-amber-700 text-white px-3 py-1.5 rounded transition-colors"
              >
                前往配置敏感词库 →
              </button>
            </div>
          ) : (
            // 已配置敏感词库 - 显示正常状态
            <>
              <div className="flex items-center justify-between mb-1 bg-blue-50 px-2 py-1.5 rounded">
                <div className="flex items-center gap-1.5">
                  <Label className="text-sm font-normal text-blue-900">
                    敏感词库
                  </Label>
                  <Badge variant="secondary" className="text-xs px-1 py-0 bg-blue-100">
                    自动启用
                  </Badge>
                  <Badge variant="secondary" className="text-xs px-1 py-0 bg-green-100 text-green-700">
                    {sensitiveTermsCount} 个词条
                  </Badge>
                </div>
              </div>
              <p className="text-xs text-gray-400 ml-2 mb-2">
                自动使用敏感词库中启用的词条进行脱敏
              </p>
            </>
          )}
        </div>
        
        {custom.length > 0 && (
          <>
            <div className="border-t pt-2 mt-2">
              <p className="text-xs text-gray-400 mb-2">自定义正则规则</p>
              {custom.map((rule) => (
                <div key={rule.id} className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-1.5">
                    <Label htmlFor={rule.id} className="text-sm font-normal cursor-pointer">
                      {rule.name}
                    </Label>
                    <Badge variant="secondary" className="text-xs px-1 py-0">正则</Badge>
                  </div>
                  <Switch
                    id={rule.id}
                    checked={selectedRules.includes(rule.id)}
                    onCheckedChange={(checked) => handleToggle(rule.id, checked)}
                  />
                </div>
              ))}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}