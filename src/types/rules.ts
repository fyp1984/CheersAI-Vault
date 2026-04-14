export interface RuleGroup {
  id: string;
  name: string;
  rules: string[];
}

export interface MaskRule {
  id: string;
  name: string;
  pattern: string;
  replacement: string;
  replacement_template: string;
  use_counter: boolean;
  enabled: boolean;
  builtin: boolean;
  description?: string;
}
