/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
import test from "node:test";
import assert from "node:assert/strict";

import {
  CELL_RANGE_LIMIT_ERROR,
  MAX_CELL_OVERRIDE_CELLS,
  getMaskingStrategyOptionState,
  isSelectableMaskingStrategy,
  PLACEHOLDER_STRATEGIES,
  RETAIN_MESSAGES,
  canConfirmExcelMasking,
  cellRangeValidationError,
  hasAnyExcelMaskingRule,
  isStalePreviewResponse,
  mergeCellOverrides,
  nextSecondaryPassphraseForKeyMode,
  parseCellRange,
} from "./ExcelMaskingDialog";

const EXACT_RETAIN_WARNING = "不勾选加密留存将无法仅凭 .ecmap 还原";

test("placeholder strategies remain visible but are disabled and grey", () => {
  assert.deepEqual(PLACEHOLDER_STRATEGIES, [
    "BANK_CARD",
    "EMAIL",
    "ADDRESS",
    "COMPLIANCE_ID",
  ]);

  for (const strategy of PLACEHOLDER_STRATEGIES) {
    assert.equal(isSelectableMaskingStrategy(strategy), false);
    assert.deepEqual(getMaskingStrategyOptionState(strategy), {
      disabled: true,
      className: "text-gray-400",
    });
  }
});

test("active strategies remain selectable", () => {
  for (const strategy of [
    "FULL_MASK",
    "PHONE_MID4",
    "IDCARD_MID10",
    "BANKCARD_LAST4",
    "EMAIL_USER_MASK",
    "DEFAULT_VALUE",
    "CLEAR_COL",
  ] as const) {
    assert.equal(isSelectableMaskingStrategy(strategy), true);
    assert.deepEqual(getMaskingStrategyOptionState(strategy), {
      disabled: false,
      className: undefined,
    });
  }
});

// R2: retention must be optional. Whether masking can be confirmed depends
// only on having rules + the final confirmation checkbox, never on whether
// the encrypted source is retained.

test("hasAnyExcelMaskingRule requires at least one rule regardless of retention", () => {
  assert.equal(hasAnyExcelMaskingRule(0), false);
  assert.equal(hasAnyExcelMaskingRule(1), true);
  assert.equal(hasAnyExcelMaskingRule(5), true);
});

test("canConfirmExcelMasking allows completion with retain=false as long as rules exist and the user confirmed", () => {
  assert.equal(canConfirmExcelMasking(1, true), true);
  assert.equal(canConfirmExcelMasking(0, true), false, "no rules must block confirmation");
  assert.equal(canConfirmExcelMasking(1, false), false, "missing final confirmation must block");
  assert.equal(canConfirmExcelMasking(0, false), false);
});

// B.3: all three retention messages must state that unchecking retention
// disables path A (.ecmap-only restore) while still allowing path B
// (user-supplied original file verified by SHA-256), and must never claim
// retention is mandatory to complete masking.

for (const key of ["tab0", "confirm", "unmask_missing"] as const) {
  test(`RETAIN_MESSAGES.${key} explains path A is unavailable without retention and describes path B`, () => {
    const message = RETAIN_MESSAGES[key];
    assert.match(message, /\.ecmap/);
    assert.match(message, /路径\s*A/);
    assert.match(message, /路径\s*B/);
    assert.match(message, /SHA-256|sha-256|SHA256/i);
    assert.match(message, new RegExp(EXACT_RETAIN_WARNING));
    assert.doesNotMatch(
      message,
      /唯一路径/,
      "must not claim retention is the only way to complete masking"
    );
  });
}

// E: illegal cell-reference input must be rejected up front, with a fixed,
// safe error message that carries no internal parser detail/path/stack
// trace, while every legal single-cell/row-range/rectangle/multi-region
// form must keep parsing exactly as before.

test("parseCellRange accepts a single cell using the default sheet", () => {
  const parsed = parseCellRange("A1", "Sheet1");
  assert.deepEqual(parsed, { sheet: "Sheet1", cells: [{ row: 0, col: 0 }] });
});

test("parseCellRange accepts an explicit sheet prefix", () => {
  const parsed = parseCellRange("Sheet2!B3", "Sheet1");
  assert.deepEqual(parsed, { sheet: "Sheet2", cells: [{ row: 2, col: 1 }] });
});

test("parseCellRange accepts a rectangle range and expands every cell in it", () => {
  const parsed = parseCellRange("B3:C4", "Sheet1");
  assert.deepEqual(parsed, {
    sheet: "Sheet1",
    cells: [
      { row: 2, col: 1 },
      { row: 2, col: 2 },
      { row: 3, col: 1 },
      { row: 3, col: 2 },
    ],
  });
});

test("parseCellRange accepts a multi-region (sheet-prefixed rectangle) reference", () => {
  const parsed = parseCellRange("Sheet1!A1:B2", "OtherSheet");
  assert.deepEqual(parsed, {
    sheet: "Sheet1",
    cells: [
      { row: 0, col: 0 },
      { row: 0, col: 1 },
      { row: 1, col: 0 },
      { row: 1, col: 1 },
    ],
  });
});

test("parseCellRange rejects empty, whitespace-only, and malformed input", () => {
  assert.equal(parseCellRange("", "Sheet1"), null);
  assert.equal(parseCellRange("   ", "Sheet1"), null);
  assert.equal(parseCellRange("not-a-cell", "Sheet1"), null);
  assert.equal(parseCellRange("123", "Sheet1"), null);
  assert.equal(parseCellRange("A1:", "Sheet1"), null);
  assert.equal(parseCellRange("Sheet1!", "Sheet1"), null);
});

// R2 (architect Review, TASK-EXCEL-P0-DYNAMIC-FAILURES-CLOSEOUT-001): a
// browser dynamic repro found `Sheet1!A0` accepted and written into rule
// state as row -1, which the backend then rejected with a 400 the UI never
// showed the user. These cases must all be rejected up front, before any
// UI state is written or any request is made.

test("parseCellRange rejects a 1-based row of 0 (A0)", () => {
  assert.equal(parseCellRange("A0", "Sheet1"), null);
  assert.equal(parseCellRange("Sheet1!A0", "Sheet1"), null);
});

test("parseCellRange rejects a 1-based row of 0 with leading zeros (A00)", () => {
  assert.equal(parseCellRange("A00", "Sheet1"), null);
});

test("parseCellRange rejects a negative row (A-1)", () => {
  assert.equal(parseCellRange("A-1", "Sheet1"), null);
});

test("parseCellRange rejects an empty explicit sheet prefix", () => {
  assert.equal(parseCellRange("!A1", "Sheet1"), null);
  assert.equal(parseCellRange("   !A1", "Sheet1"), null);
});

test("parseCellRange rejects a reversed range (start after end)", () => {
  assert.equal(parseCellRange("D5:B3", "Sheet1"), null, "row and col both reversed");
  assert.equal(parseCellRange("B5:D3", "Sheet1"), null, "row reversed only");
  assert.equal(parseCellRange("D3:B5", "Sheet1"), null, "col reversed only");
  // A range collapsed to a single cell (start == end) is not reversed.
  assert.notEqual(parseCellRange("B3:B3", "Sheet1"), null);
});

test("parseCellRange rejects a range with a missing endpoint", () => {
  assert.equal(parseCellRange("A1:", "Sheet1"), null, "missing end");
  assert.equal(parseCellRange(":B3", "Sheet1"), null, "missing start");
  assert.equal(parseCellRange("A0:B3", "Sheet1"), null, "illegal start with a legal end");
  assert.equal(parseCellRange("A1:B0", "Sheet1"), null, "legal start with an illegal end");
});

test("parseCellRange keeps accepting every legal single-cell/range/multi-region form (no regression)", () => {
  assert.notEqual(parseCellRange("A1", "Sheet1"), null);
  assert.notEqual(parseCellRange("B3:D5", "Sheet1"), null);
  assert.notEqual(parseCellRange("Sheet1!A2:B3", "OtherSheet"), null);
  assert.notEqual(parseCellRange("Sheet2!B3", "Sheet1"), null);
});

test("parseCellRange accepts exactly 10,000 cells and rejects 10,001 before expansion", () => {
  const boundary = parseCellRange("Sheet1!A1:A10000", "OtherSheet");
  assert.notEqual(boundary, null);
  assert.equal(boundary?.cells.length, MAX_CELL_OVERRIDE_CELLS);
  assert.equal(parseCellRange("A1:A10001", "Sheet1"), null);
  assert.equal(parseCellRange("A1:Z1000000", "Sheet1"), null);
});

test("oversized CellRef uses the fixed 10,000 limit message", () => {
  assert.equal(
    cellRangeValidationError("A1:A10001", "Sheet1"),
    CELL_RANGE_LIMIT_ERROR
  );
  assert.match(CELL_RANGE_LIMIT_ERROR, /10,000/);
  assert.doesNotMatch(CELL_RANGE_LIMIT_ERROR, /stack|Error|\//i);
});

test("mergeCellOverrides replaces a 10,000-cell range without duplicates", () => {
  const parsed = parseCellRange("A1:CV100", "Sheet1");
  assert.notEqual(parsed, null);
  assert.equal(parsed?.cells.length, MAX_CELL_OVERRIDE_CELLS);

  const existing = (parsed?.cells ?? []).map((cell) => ({
    sheet: "Sheet1",
    row: cell.row,
    col: cell.col,
    strategy: "FULL_MASK" as const,
    replacement: undefined,
  }));
  const merged = mergeCellOverrides(
    existing,
    parsed!,
    "PHONE_MID4",
    "fixture-replacement"
  );

  assert.equal(merged.length, MAX_CELL_OVERRIDE_CELLS);
  assert.equal(
    new Set(merged.map((rule) => `${rule.sheet}:${rule.row}:${rule.col}`)).size,
    MAX_CELL_OVERRIDE_CELLS
  );
  assert.equal(merged[0].strategy, "PHONE_MID4");
  assert.equal(merged[merged.length - 1]?.replacement, "fixture-replacement");
});

test("cellRangeValidationError rejects every R2 illegal form with the fixed safe message and no leaked detail", () => {
  for (const illegal of ["A0", "A00", "A-1", "!A1", "D5:B3", "A1:", ":B3"]) {
    const message = cellRangeValidationError(illegal, "Sheet1");
    assert.notEqual(message, null, `expected an error for: ${illegal}`);
    assert.match(message as string, /单元格引用格式不正确/);
    assert.doesNotMatch(message as string, /at \S+:\d+/);
    assert.doesNotMatch(message as string, /\//);
    assert.doesNotMatch(message as string, /Error/i);
  }
});

test("cellRangeValidationError is null while the input is empty (no error shown before the user types)", () => {
  assert.equal(cellRangeValidationError("", "Sheet1"), null);
  assert.equal(cellRangeValidationError("   ", "Sheet1"), null);
});

test("cellRangeValidationError is null for legal single-cell, range, and sheet-prefixed input", () => {
  assert.equal(cellRangeValidationError("A1", "Sheet1"), null);
  assert.equal(cellRangeValidationError("B3:D5", "Sheet1"), null);
  assert.equal(cellRangeValidationError("Sheet1!A2:B3", "OtherSheet"), null);
});

test("cellRangeValidationError returns a fixed, safe message for illegal non-empty input", () => {
  const message = cellRangeValidationError("not-a-cell", "Sheet1");
  assert.notEqual(message, null);
  assert.match(message as string, /单元格引用格式不正确/);
  // Must not leak internal parser detail, a file path, or a stack trace.
  assert.doesNotMatch(message as string, /at \S+:\d+/);
  assert.doesNotMatch(message as string, /\//);
  assert.doesNotMatch(message as string, /Error/i);
});

test("cellRangeValidationError clears the moment invalid input is corrected to a valid one", () => {
  assert.notEqual(cellRangeValidationError("A", "Sheet1"), null);
  assert.equal(cellRangeValidationError("A1", "Sheet1"), null);
});

// B.2: a fast repeated preview refresh must not let an earlier (slower)
// response overwrite a newer one.

test("isStalePreviewResponse is false when the response's request id is still the latest", () => {
  assert.equal(isStalePreviewResponse(3, 3), false);
});

test("isStalePreviewResponse is true once a newer request has since been issued", () => {
  assert.equal(isStalePreviewResponse(1, 2), true, "an older in-flight request must be discarded");
  assert.equal(isStalePreviewResponse(2, 5), true);
});

// AC-14: switching key mode away from SECONDARY_PASSPHRASE must clear the
// independent secondary passphrase immediately in memory; switching back
// must start empty, never resurface a value typed in a previous visit.

test("nextSecondaryPassphraseForKeyMode clears the value when switching to SANDBOX_REUSED", () => {
  assert.equal(
    nextSecondaryPassphraseForKeyMode("SANDBOX_REUSED", "fixture-secondary-test-pass"),
    ""
  );
});

test("nextSecondaryPassphraseForKeyMode clears the value when switching to DEVICE_KEY", () => {
  assert.equal(
    nextSecondaryPassphraseForKeyMode("DEVICE_KEY", "fixture-secondary-test-pass"),
    ""
  );
});

test("nextSecondaryPassphraseForKeyMode leaves the value untouched while staying in SECONDARY_PASSPHRASE", () => {
  assert.equal(
    nextSecondaryPassphraseForKeyMode("SECONDARY_PASSPHRASE", "fixture-secondary-test-pass"),
    "fixture-secondary-test-pass"
  );
});

test("nextSecondaryPassphraseForKeyMode returns empty for an already-empty value regardless of mode", () => {
  assert.equal(nextSecondaryPassphraseForKeyMode("SANDBOX_REUSED", ""), "");
  assert.equal(nextSecondaryPassphraseForKeyMode("SECONDARY_PASSPHRASE", ""), "");
});

test("nextSecondaryPassphraseForKeyMode simulates a full switch-away-and-back cycle ending empty", () => {
  let secondaryPassphrase = "";
  secondaryPassphrase = nextSecondaryPassphraseForKeyMode("SECONDARY_PASSPHRASE", secondaryPassphrase);
  // user types into the field while in SECONDARY_PASSPHRASE mode
  secondaryPassphrase = "fixture-secondary-test-pass-001";
  // switch to mode① (sandbox reused) — must clear immediately
  secondaryPassphrase = nextSecondaryPassphraseForKeyMode("SANDBOX_REUSED", secondaryPassphrase);
  assert.equal(secondaryPassphrase, "");
  // switch back to mode② — must start empty, not resurface the old value
  secondaryPassphrase = nextSecondaryPassphraseForKeyMode("SECONDARY_PASSPHRASE", secondaryPassphrase);
  assert.equal(secondaryPassphrase, "");
});
