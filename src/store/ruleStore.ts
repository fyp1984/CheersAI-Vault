import { create } from "zustand";
import type { MaskRule } from "@/types/rules";

const DEFAULT_RULES: MaskRule[] = [
  { id: "id_card", name: "身份证号", pattern: "", replacement: "***IDCARD***", replacement_template: "***IDCARD***", use_counter: true, enabled: true, builtin: true, description: "匹配18位中国居民身份证号" },
  { id: "phone", name: "手机号", pattern: "", replacement: "***PHONE***", replacement_template: "***PHONE***", use_counter: true, enabled: true, builtin: true, description: "匹配1开头的11位手机号" },
  { id: "email", name: "电子邮箱", pattern: "", replacement: "***EMAIL***", replacement_template: "***EMAIL***", use_counter: true, enabled: true, builtin: true, description: "匹配标准电子邮件地址" },
  { id: "bank_card", name: "银行卡号", pattern: "", replacement: "***BANKCARD***", replacement_template: "***BANKCARD***", use_counter: true, enabled: true, builtin: true, description: "匹配16-19位银行卡号" },
  { id: "ipv4", name: "IPv4地址", pattern: "", replacement: "***IP***", replacement_template: "***IP***", use_counter: true, enabled: true, builtin: true, description: "匹配IPv4格式IP地址" },
  { id: "chinese_name", name: "中文姓名", pattern: "", replacement: "***NAME***", replacement_template: "姓名", use_counter: true, enabled: false, builtin: true, description: "匹配2-4个汉字的中文姓名（可能误报）" },
  { id: "passport", name: "护照号", pattern: "", replacement: "***PASSPORT***", replacement_template: "***PASSPORT***", use_counter: true, enabled: true, builtin: true, description: "匹配中国护照号（字母+8位数字）" },
];

interface RuleStore {
  rules: MaskRule[];
  selectedRuleIds: string[];
  setRules: (rules: MaskRule[]) => void;
  toggleRule: (id: string) => void;
  addRule: (rule: MaskRule) => void;
  removeRule: (id: string) => void;
  setSelectedRuleIds: (ids: string[]) => void;
}

export const useRuleStore = create<RuleStore>((set) => ({
  rules: DEFAULT_RULES,
  selectedRuleIds: DEFAULT_RULES.filter((r) => r.enabled).map((r) => r.id),

  setRules: (rules) => set({ rules }),

  toggleRule: (id) =>
    set((state) => ({
      rules: state.rules.map((r) =>
        r.id === id ? { ...r, enabled: !r.enabled } : r
      ),
    })),

  addRule: (rule) =>
    set((state) => ({ rules: [...state.rules, rule] })),

  removeRule: (id) =>
    set((state) => ({ rules: state.rules.filter((r) => r.id !== id) })),

  setSelectedRuleIds: (ids) => set({ selectedRuleIds: ids }),
}));
