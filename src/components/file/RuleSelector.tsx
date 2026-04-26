import { useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { useRuleStore } from "@/store/ruleStore";

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

  // 初始化时恢复 localStorage 选择或使用默认启用规则
  useEffect(() => {
    // 只在规则加载完成且还没有选择时初始化
    if (rules.length === 0) return;
    if (selectedRules.length > 0) return;
    
    try {
      const saved = localStorage.getItem("selected-rules");
      const savedIds: string[] = saved ? JSON.parse(saved) : [];
      const valid = savedIds.filter((id) => rules.some((r) => r.id === id));
      if (valid.length > 0) { 
        console.log("Restoring saved rules:", valid);
        onRulesChange(valid); 
        return; 
      }
    } catch { /* ignore */ }
    
    // 默认选择所有启用的规则
    const defaultRules = rules.filter((r) => r.enabled).map((r) => r.id);
    console.log("Using default enabled rules:", defaultRules);
    onRulesChange(defaultRules);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rules]);

  const handleToggle = (ruleId: string, checked: boolean) => {
    const next = checked
      ? [...selectedRules, ruleId]
      : selectedRules.filter((id) => id !== ruleId);
    onRulesChange(next);
    saveRules(next);
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
        {custom.length > 0 && (
          <>
            <div className="border-t pt-2 mt-2">
              <p className="text-xs text-gray-400 mb-2">自定义规则</p>
              {custom.map((rule) => (
                <div key={rule.id} className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-1.5">
                    <Label htmlFor={rule.id} className="text-sm font-normal cursor-pointer">
                      {rule.name}
                    </Label>
                    <Badge variant="secondary" className="text-xs px-1 py-0">自定义</Badge>
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