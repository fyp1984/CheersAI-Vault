import test from "node:test";
import assert from "node:assert/strict";

import { useRuleStore } from "./ruleStore";

test("中文姓名规则默认不启用且不加入默认选择", () => {
  const { rules, selectedRuleIds } = useRuleStore.getState();
  const chineseNameRule = rules.find((rule) => rule.id === "chinese_name");

  assert.ok(chineseNameRule, "should expose the chinese_name rule");
  assert.equal(chineseNameRule.enabled, false);
  assert.equal(
    selectedRuleIds.includes("chinese_name"),
    false,
    "selectedRuleIds should exclude chinese_name by default"
  );
});
