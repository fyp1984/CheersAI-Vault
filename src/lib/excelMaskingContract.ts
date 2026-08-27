/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
/**
 * Pure DTO adapter between the canonical Excel masking config (used by the
 * browser Runtime and by the frontend store/UI) and the Tauri desktop
 * command's native DTO shape. No network or key-derivation logic lives here.
 */
import type {
  CellOverrideRule,
  ColumnMaskRule,
  EncSourceKeyMode,
  ExcelMaskingConfig,
  ExcelMaskPreview,
  MaskingStrategyId,
} from "@/types/commands";

export const CANONICAL_MASKING_STRATEGY_IDS: readonly MaskingStrategyId[] = [
  "FULL_MASK",
  "PHONE_MID4",
  "IDCARD_MID10",
  "BANKCARD_LAST4",
  "EMAIL_USER_MASK",
  "DEFAULT_VALUE",
  "CLEAR_COL",
  "BANK_CARD",
  "EMAIL",
  "ADDRESS",
  "COMPLIANCE_ID",
];

const STRATEGY_ID_SET = new Set<string>(CANONICAL_MASKING_STRATEGY_IDS);

export const CANONICAL_KEY_MODES: readonly EncSourceKeyMode[] = [
  "SANDBOX_REUSED",
  "SECONDARY_PASSPHRASE",
  "DEVICE_KEY",
];

const KEY_MODE_SET = new Set<string>(CANONICAL_KEY_MODES);

export class ExcelMaskingContractError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ExcelMaskingContractError";
  }
}

export type TauriEncSourcePassModeDto =
  | { type: "SandboxReused" }
  | { type: "SecondaryPhrase"; value: string }
  | { type: "DeviceKey" };

/**
 * Explicit, backward-compatible rule-contract marker (Tauri native
 * `rule_mode`). Every rule this adapter produces carries this marker so the
 * Tauri host masks it via `strategy_id`/`replacement`; a native config
 * written before this field existed omits it entirely and is masked with
 * the legacy `pattern`/`mask_char`/`keep_prefix`/`keep_suffix` algorithm
 * instead — field presence alone (e.g. a bare `replacement`) cannot tell a
 * legacy rule apart from a canonical one, so this marker is load-bearing.
 */
export const TAURI_RULE_MODE_CANONICAL = "CANONICAL" as const;

export interface TauriColumnMaskingRule {
  strategy_id: string;
  pattern?: string;
  replacement?: string;
  mask_char?: string;
  keep_prefix?: number;
  keep_suffix?: number;
  enabled: boolean;
  rule_mode: typeof TAURI_RULE_MODE_CANONICAL;
}

export interface TauriCellOverride {
  cell_ref: string;
  strategy_id: string;
  replacement?: string;
}

export interface TauriSheetMaskingConfig {
  sheet_name: string;
  header_row?: number;
  column_rules: Record<string, TauriColumnMaskingRule>;
  cell_overrides: TauriCellOverride[];
}

export interface TauriExcelMaskingConfig {
  input_file_path: string;
  output_name_suffix?: string;
  sheets: TauriSheetMaskingConfig[];
  passphrase?: string;
  retain_encrypted_source?: boolean;
  source_pass_mode: TauriEncSourcePassModeDto;
  generate_ecmap: boolean;
}

/**
 * Native Tauri preview DTO. Rust returns one entry per non-empty cell grouped
 * under each sheet, while the frontend contract consumes one entry per row.
 */
export interface TauriExcelMaskPreviewCell {
  original_preview: string;
  masked: string;
  strategy_id: string;
  row: number;
  col: number;
  cell_ref: string;
}

export interface TauriSheetMaskPreview {
  sheet_name: string;
  headers: string[];
  preview_rows: TauriExcelMaskPreviewCell[];
}

export interface TauriExcelMaskPreview {
  sheets: TauriSheetMaskPreview[];
}

export interface ExcelMaskingAdapterOptions {
  /**
   * The already-known sandbox passphrase (fileStore.passphrase). Required so
   * SANDBOX_REUSED mode encrypts with the same key the rest of the app uses;
   * ignored by the native side for DEVICE_KEY.
   */
  sandboxPassphrase?: string;
}

function columnIndexToLetters(index0: number): string {
  let n = index0 + 1;
  let out = "";
  while (n > 0) {
    const rem = (n - 1) % 26;
    out = String.fromCharCode(65 + rem) + out;
    n = Math.floor((n - 1) / 26);
  }
  return out;
}

function toCellRef(row0: number, col0: number): string {
  return `${columnIndexToLetters(col0)}${row0 + 1}`;
}

const INVALID_TAURI_PREVIEW_MESSAGE = "Tauri Excel 预览返回结构无效，拒绝继续";

function rejectInvalidTauriPreview(): never {
  // Keep this error deliberately value-free: the native response can contain
  // original cell previews, which must never be copied into an error message.
  throw new ExcelMaskingContractError(INVALID_TAURI_PREVIEW_MESSAGE);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readPreviewString(value: unknown): string {
  if (typeof value !== "string") rejectInvalidTauriPreview();
  return value;
}

function readPreviewInteger(value: unknown, minimum: number): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < minimum
  ) {
    rejectInvalidTauriPreview();
  }
  return value;
}

function nativeCellRef(row: number, col: number): string {
  return `${columnIndexToLetters(col - 1)}${row}`;
}

function readNativePreviewCell(value: unknown): TauriExcelMaskPreviewCell {
  if (!isRecord(value)) rejectInvalidTauriPreview();

  const original_preview = readPreviewString(value.original_preview);
  const masked = readPreviewString(value.masked);
  const strategy_id = readPreviewString(value.strategy_id);
  const row = readPreviewInteger(value.row, 1);
  const col = readPreviewInteger(value.col, 1);
  const cell_ref = readPreviewString(value.cell_ref);

  if (
    strategy_id.trim().length === 0 ||
    cell_ref.trim().length === 0 ||
    cell_ref.toUpperCase() !== nativeCellRef(row, col)
  ) {
    rejectInvalidTauriPreview();
  }

  return { original_preview, masked, strategy_id, row, col, cell_ref };
}

function readNativePreviewSheet(value: unknown): TauriSheetMaskPreview {
  if (!isRecord(value)) rejectInvalidTauriPreview();

  const sheet_name = readPreviewString(value.sheet_name);
  if (sheet_name.trim().length === 0 || !Array.isArray(value.headers)) {
    rejectInvalidTauriPreview();
  }

  const headers = value.headers.map((header) => {
    if (typeof header !== "string") rejectInvalidTauriPreview();
    return header;
  });

  if (!Array.isArray(value.preview_rows)) rejectInvalidTauriPreview();
  const preview_rows = value.preview_rows.map(readNativePreviewCell);

  for (const cell of preview_rows) {
    if (cell.col > headers.length) rejectInvalidTauriPreview();
  }

  return { sheet_name, headers, preview_rows };
}

/**
 * Converts the real native Tauri preview shape into the canonical frontend
 * shape. Native `row` is the legacy header-inclusive range row, so the first
 * data row is 1 there but file/canonical `row_index` is 2. Native cells are
 * sorted before grouping so response order cannot change the rendered result.
 */
export function toCanonicalExcelMaskPreview(
  response: unknown
): ExcelMaskPreview {
  if (!isRecord(response) || !Array.isArray(response.sheets)) {
    rejectInvalidTauriPreview();
  }

  const sheets = response.sheets.map(readNativePreviewSheet);
  const seenSheetNames = new Set<string>();
  const preview_rows: ExcelMaskPreview["preview_rows"] = [];

  for (const sheet of sheets) {
    const sheetKey = sheet.sheet_name.toLowerCase();
    if (seenSheetNames.has(sheetKey)) rejectInvalidTauriPreview();
    seenSheetNames.add(sheetKey);

    const seenCells = new Set<string>();
    const rows = new Map<
      number,
      { original_preview: (string | null)[]; masked: string[] }
    >();
    const cells = [...sheet.preview_rows].sort(
      (left, right) => left.row - right.row || left.col - right.col
    );

    for (const cell of cells) {
      const cellKey = `${cell.row}:${cell.col}`;
      if (seenCells.has(cellKey)) rejectInvalidTauriPreview();
      seenCells.add(cellKey);

      let row = rows.get(cell.row);
      if (!row) {
        row = {
          original_preview: new Array<string | null>(sheet.headers.length).fill(null),
          masked: new Array<string>(sheet.headers.length).fill(""),
        };
        rows.set(cell.row, row);
      }

      const columnIndex = cell.col - 1;
      row.original_preview[columnIndex] = cell.original_preview;
      row.masked[columnIndex] = cell.masked;
    }

    for (const [nativeRow, row] of [...rows.entries()].sort(
      ([left], [right]) => left - right
    )) {
      preview_rows.push({
        original_preview: row.original_preview,
        masked: row.masked,
        row_index: nativeRow + 1,
        sheet: sheet.sheet_name,
      });
    }
  }

  return { preview_rows, conflicts: [] };
}

function assertKnownStrategy(strategy: string): void {
  if (!STRATEGY_ID_SET.has(strategy)) {
    throw new ExcelMaskingContractError(`未知脱敏策略，拒绝转换：${strategy}`);
  }
}

function toSourcePassMode(
  keyMode: EncSourceKeyMode,
  secondaryPassphrase: string | undefined
): TauriEncSourcePassModeDto {
  switch (keyMode) {
    case "SANDBOX_REUSED":
      return { type: "SandboxReused" };
    case "DEVICE_KEY":
      return { type: "DeviceKey" };
    case "SECONDARY_PASSPHRASE": {
      const phrase = secondaryPassphrase?.trim();
      if (!phrase) {
        throw new ExcelMaskingContractError("独立二级口令不能为空");
      }
      return { type: "SecondaryPhrase", value: phrase };
    }
    default:
      throw new ExcelMaskingContractError(
        `未知密钥来源模式，拒绝转换：${keyMode as string}`
      );
  }
}

function toColumnRule(rule: ColumnMaskRule): TauriColumnMaskingRule {
  assertKnownStrategy(rule.strategy);
  return {
    strategy_id: rule.strategy,
    replacement: rule.replacement,
    enabled: true,
    rule_mode: TAURI_RULE_MODE_CANONICAL,
  };
}

function toCellOverride(rule: CellOverrideRule): TauriCellOverride {
  assertKnownStrategy(rule.strategy);
  return {
    cell_ref: toCellRef(rule.row, rule.col),
    strategy_id: rule.strategy,
    replacement: rule.replacement,
  };
}

/**
 * Converts the canonical (browser-Runtime-equivalent) Excel masking config
 * into the Tauri command's native DTO. Throws ExcelMaskingContractError on
 * any unknown strategy/key-mode or unresolvable ambiguity instead of
 * silently dropping or downgrading rules.
 */
export function toTauriExcelMaskingConfig(
  config: ExcelMaskingConfig,
  options: ExcelMaskingAdapterOptions = {}
): TauriExcelMaskingConfig {
  if (!config.file_path) {
    throw new ExcelMaskingContractError("file_path 不能为空");
  }
  if (!KEY_MODE_SET.has(config.key_mode)) {
    throw new ExcelMaskingContractError(
      `未知密钥来源模式，拒绝转换：${config.key_mode as string}`
    );
  }

  const sheets: TauriSheetMaskingConfig[] = config.sheet_policies.map((policy) => {
    const column_rules: Record<string, TauriColumnMaskingRule> = {};
    const headerToColIndex = new Map<string, number>();
    for (const rule of policy.column_rules) {
      const existingColIndex = headerToColIndex.get(rule.headerText);
      if (existingColIndex !== undefined && existingColIndex !== rule.colIndex) {
        throw new ExcelMaskingContractError(
          `工作表 '${policy.sheet}' 的表头 '${rule.headerText}' 存在重复列，无法转换为 Tauri 原生配置`
        );
      }
      headerToColIndex.set(rule.headerText, rule.colIndex);
      column_rules[rule.headerText] = toColumnRule(rule);
    }
    return {
      sheet_name: policy.sheet,
      header_row: 0,
      column_rules,
      cell_overrides: policy.cell_overrides.map(toCellOverride),
    };
  });

  const source_pass_mode = toSourcePassMode(config.key_mode, config.secondary_passphrase);
  if (
    config.key_mode === "SANDBOX_REUSED" &&
    !options.sandboxPassphrase?.trim()
  ) {
    throw new ExcelMaskingContractError("沙箱口令不能为空");
  }

  return {
    input_file_path: config.file_path,
    sheets,
    passphrase: options.sandboxPassphrase ?? "",
    retain_encrypted_source: config.retain_encrypted_source,
    source_pass_mode,
    generate_ecmap: true,
  };
}
