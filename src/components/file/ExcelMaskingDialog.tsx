/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  Check,
  ChevronDown,
  Eye,
  EyeOff,
  Grid3X3,
  Info,
  Key,
  Lock,
  RefreshCw,
  Shield,
  Sparkles,
  Table2,
  X,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Card, CardContent } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Progress } from "@/components/ui/progress";
import { PassphraseBox } from "@/components/common/PassphraseBox";
import { tauriCommands } from "@/lib/tauri";
import { useExcelMaskingStore } from "@/store/excelMaskingStore";
import type {
  CellOverrideRule,
  ColumnMaskRule,
  EncSourceKeyMode,
  ExcelMaskingConfig,
  ExcelMaskPreview,
  MaskingStrategyId,
  SheetDef,
} from "@/types/commands";
import { cn } from "@/lib/utils";

export const RETAIN_MESSAGES = {
  tab0: "保留加密源是启用自动反脱敏的唯一路径；若不勾选将无法自动还原原始 xlsx 样式；用户原件路径 B 也可完成但要求原件 sha 完全匹配",
  confirm: "保留加密源是启用自动反脱敏的唯一路径；若不勾选将无法自动还原原始 xlsx 样式；用户原件路径 B 也可完成但要求原件 sha 完全匹配",
  unmask_missing: "保留加密源是启用自动反脱敏的唯一路径；若不勾选将无法自动还原原始 xlsx 样式；用户原件路径 B 也可完成但要求原件 sha 完全匹配",
};

const PLACEHOLDER_STRATEGIES: MaskingStrategyId[] = [
  "BANK_CARD",
  "EMAIL",
  "ADDRESS",
  "COMPLIANCE_ID",
];

const STRATEGY_LABELS: Record<MaskingStrategyId, string> = {
  FULL_MASK: "全掩码（***）",
  PHONE_MID4: "手机中间 4 位",
  IDCARD_MID10: "身份证中间 10 位",
  BANKCARD_LAST4: "银行卡后 4 位",
  EMAIL_USER_MASK: "邮箱用户名掩码",
  DEFAULT_VALUE: "默认值替换",
  CLEAR_COL: "清空整列",
  BANK_CARD: "银行卡（预留占位）",
  EMAIL: "邮箱（预留占位）",
  ADDRESS: "地址（预留占位）",
  COMPLIANCE_ID: "合规 ID（预留占位）",
};

const ACTIVE_STRATEGIES: MaskingStrategyId[] = [
  "FULL_MASK",
  "PHONE_MID4",
  "IDCARD_MID10",
  "BANKCARD_LAST4",
  "EMAIL_USER_MASK",
  "DEFAULT_VALUE",
  "CLEAR_COL",
];

interface ExcelMaskingDialogProps {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  filePaths: string[];
  onCancel: () => void;
  onConfirm: (
    configs: ExcelMaskingConfig[],
    outputDirOverride?: string
  ) => void;
  defaultPassphrase?: string;
  defaultOutputDir?: string;
}

function isExcelFile(p: string): boolean {
  const lower = p.toLowerCase();
  return (
    lower.endsWith(".xlsx") ||
    lower.endsWith(".xls") ||
    lower.endsWith(".xlsm") ||
    lower.endsWith(".csv")
  );
}

export default function ExcelMaskingDialog({
  open,
  onOpenChange,
  filePaths,
  onCancel,
  onConfirm,
  defaultPassphrase = "",
  defaultOutputDir,
}: ExcelMaskingDialogProps) {
  const excelPaths = useMemo(
    () => filePaths.filter(isExcelFile),
    [filePaths]
  );

  const primaryPath = excelPaths[0] ?? "";
  const { privacy } = useExcelMaskingStore();

  const [activeTab, setActiveTab] = useState("retain");
  const [loading, setLoading] = useState(false);
  const [loadingProgress, setLoadingProgress] = useState(0);
  const [sheets, setSheets] = useState<SheetDef[]>([]);
  const [selectedSheet, setSelectedSheet] = useState<string>("");

  const [retainChecked, setRetainChecked] = useState(
    privacy.excelDefaultRetainEncryptedSource
  );
  const [keyMode, setKeyMode] = useState<EncSourceKeyMode>("SANDBOX_REUSED");
  const [secondaryPassphrase, setSecondaryPassphrase] = useState(
    defaultPassphrase
  );

  const [columnRules, setColumnRules] = useState<ColumnMaskRule[]>([]);
  const [cellOverrides, setCellOverrides] = useState<CellOverrideRule[]>([]);

  const [overrideCellInput, setOverrideCellInput] = useState("");
  const [overrideStrategy, setOverrideStrategy] =
    useState<MaskingStrategyId>("FULL_MASK");
  const [overrideReplacement, setOverrideReplacement] = useState("");

  const [preview, setPreview] = useState<ExcelMaskPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  const [confirmSecondCheck, setConfirmSecondCheck] = useState(false);

  useEffect(() => {
    if (!open || !primaryPath) return;
    let cancelled = false;
    setLoading(true);
    setLoadingProgress(10);
    tauriCommands
      .excelParseStructure(primaryPath)
      .then((defs) => {
        if (cancelled) return;
        setSheets(defs);
        setSelectedSheet(defs[0]?.name ?? "");
        setLoadingProgress(100);
      })
      .catch(() => {
        if (cancelled) return;
        setSheets([]);
      })
      .finally(() => {
        if (cancelled) return;
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, primaryPath]);

  useEffect(() => {
    if (!privacy.excelDefaultRetainEncryptedSource) return;
    setRetainChecked(false);
  }, [open, privacy.excelDefaultRetainEncryptedSource]);

  const currentSheetDef = useMemo(
    () => sheets.find((s) => s.name === selectedSheet),
    [sheets, selectedSheet]
  );

  const checkedColIndices = useMemo(() => {
    return new Set(
      columnRules
        .filter((r) => r.sheet === selectedSheet)
        .map((r) => r.colIndex)
    );
  }, [columnRules, selectedSheet]);

  const rulesCount = useMemo(() => {
    return columnRules.length + cellOverrides.length;
  }, [columnRules, cellOverrides]);

  const canConfirm = retainChecked && rulesCount > 0 && confirmSecondCheck;

  const toggleColumnChecked = useCallback(
    (colIndex: number, headerText: string) => {
      setColumnRules((prev) => {
        const existing = prev.find(
          (r) => r.sheet === selectedSheet && r.colIndex === colIndex
        );
        if (existing) {
          return prev.filter((r) => r !== existing);
        }
        return [
          ...prev,
          {
            sheet: selectedSheet,
            colIndex,
            headerText,
            strategy: "FULL_MASK" as MaskingStrategyId,
          },
        ];
      });
    },
    [selectedSheet]
  );

  const updateColumnRule = useCallback(
    (
      colIndex: number,
      patch: Partial<Pick<ColumnMaskRule, "strategy" | "replacement">>
    ) => {
      setColumnRules((prev) =>
        prev.map((r) =>
          r.sheet === selectedSheet && r.colIndex === colIndex
            ? { ...r, ...patch }
            : r
        )
      );
    },
    [selectedSheet]
  );

  const parseCellRange = (
    input: string
  ): { sheet: string; cells: { row: number; col: number }[] } | null => {
    const trimmed = input.trim();
    if (!trimmed) return null;

    const colLetterToIndex = (letters: string): number => {
      let result = 0;
      for (let i = 0; i < letters.length; i++) {
        result = result * 26 + (letters.charCodeAt(i) - 64);
      }
      return result - 1;
    };

    const parseRef = (ref: string): { col: number; row: number } | null => {
      const m = ref.match(/^([A-Za-z]+)(\d+)$/);
      if (!m) return null;
      return {
        col: colLetterToIndex(m[1].toUpperCase()),
        row: parseInt(m[2], 10) - 1,
      };
    };

    const sepIdx = trimmed.indexOf("!");
    let sheetName = selectedSheet;
    let refPart = trimmed;
    if (sepIdx >= 0) {
      sheetName = trimmed.slice(0, sepIdx);
      refPart = trimmed.slice(sepIdx + 1);
    }

    const colonIdx = refPart.indexOf(":");
    if (colonIdx >= 0) {
      const start = parseRef(refPart.slice(0, colonIdx));
      const end = parseRef(refPart.slice(colonIdx + 1));
      if (!start || !end) return null;
      const cells: { row: number; col: number }[] = [];
      for (let r = start.row; r <= end.row; r++) {
        for (let c = start.col; c <= end.col; c++) {
          cells.push({ row: r, col: c });
        }
      }
      return { sheet: sheetName, cells };
    }
    const single = parseRef(refPart);
    if (!single) return null;
    return { sheet: sheetName, cells: [single] };
  };

  const addCellOverride = useCallback(() => {
    const parsed = parseCellRange(overrideCellInput);
    if (!parsed) return;
    setCellOverrides((prev) => {
      let next = [...prev];
      for (const cell of parsed.cells) {
        next = next.filter(
          (o) =>
            !(
              o.sheet === parsed.sheet &&
              o.row === cell.row &&
              o.col === cell.col
            )
        );
        next.push({
          sheet: parsed.sheet,
          row: cell.row,
          col: cell.col,
          strategy: overrideStrategy,
          replacement: overrideReplacement || undefined,
        });
      }
      return next;
    });
    setOverrideCellInput("");
    setOverrideReplacement("");
  }, [overrideCellInput, overrideStrategy, overrideReplacement, selectedSheet]);

  const removeCellOverride = useCallback((idx: number) => {
    setCellOverrides((prev) => prev.filter((_, i) => i !== idx));
  }, []);

  const conflictCells = useMemo(() => {
    const set = new Set<string>();
    for (const cr of columnRules) {
      set.add(`${cr.sheet}::col:${cr.colIndex}`);
    }
    const result = new Set<string>();
    for (const co of cellOverrides) {
      if (set.has(`${co.sheet}::col:${co.col}`)) {
        result.add(`${co.sheet}!${co.row}:${co.col}`);
      }
    }
    return result;
  }, [columnRules, cellOverrides]);

  const loadPreview = useCallback(async () => {
    if (!primaryPath) return;
    setPreviewLoading(true);
    try {
      const config: ExcelMaskingConfig = buildConfigs()[0] ?? {
        file_path: primaryPath,
        sheet_policies: [],
        retain_encrypted_source: retainChecked,
        key_mode: keyMode,
        secondary_passphrase:
          keyMode === "SECONDARY_PASSPHRASE" ? secondaryPassphrase : undefined,
      };
      const res = await tauriCommands.excelPreviewMasking(config, 20);
      setPreview(res);
    } catch {
      setPreview(null);
    } finally {
      setPreviewLoading(false);
    }
  }, [primaryPath, retainChecked, keyMode, secondaryPassphrase]);

  const buildConfigs = useCallback((): ExcelMaskingConfig[] => {
    const policies = sheets.map((s) => {
      return {
        sheet: s.name,
        column_rules: columnRules.filter((r) => r.sheet === s.name),
        cell_overrides: cellOverrides.filter((o) => o.sheet === s.name),
      };
    });

    return excelPaths.map((fp) => ({
      file_path: fp,
      sheet_policies: policies,
      retain_encrypted_source: retainChecked,
      key_mode: keyMode,
      secondary_passphrase:
        keyMode === "SECONDARY_PASSPHRASE" ? secondaryPassphrase : undefined,
    }));
  }, [
    sheets,
    columnRules,
    cellOverrides,
    excelPaths,
    retainChecked,
    keyMode,
    secondaryPassphrase,
  ]);

  const handleConfirm = useCallback(async () => {
    if (!canConfirm) return;
    const configs = buildConfigs();
    onConfirm(configs, defaultOutputDir);
    onOpenChange(false);
  }, [canConfirm, buildConfigs, onConfirm, defaultOutputDir, onOpenChange]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-5xl max-h-[90vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield className="w-5 h-5 text-blue-500" />
            Excel 脱敏配置
            {excelPaths.length > 1 && (
              <span className="ml-2 text-xs bg-blue-100 text-blue-700 px-2 py-0.5 rounded">
                批量 {excelPaths.length} 个文件
              </span>
            )}
          </DialogTitle>
          <DialogDescription>
            配置列策略、单元格覆盖规则和加密源选项；当前以第一个文件预览结构，批量文件应用同一套规则。
          </DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="py-12 px-6 space-y-4">
            <Progress value={loadingProgress} />
            <p className="text-xs text-gray-500 text-center">
              正在解析 Excel 结构...
            </p>
          </div>
        ) : (
          <Tabs
            value={activeTab}
            onValueChange={setActiveTab}
            className="flex-1 flex flex-col overflow-hidden"
          >
            <TabsList className="mx-6 shrink-0">
              <TabsTrigger value="retain">加密源 &amp; 密钥</TabsTrigger>
              <TabsTrigger value="columns">列策略</TabsTrigger>
              <TabsTrigger value="cells">单元格覆盖</TabsTrigger>
              <TabsTrigger value="preview">预览 &amp; 应用</TabsTrigger>
            </TabsList>

            <div className="flex-1 overflow-auto px-6 py-2">
              <TabsContent value="retain" className="mt-0 h-full">
                <EncRetainTab
                  retainChecked={retainChecked}
                  onRetainChange={setRetainChecked}
                  keyMode={keyMode}
                  onKeyModeChange={setKeyMode}
                  secondaryPassphrase={secondaryPassphrase}
                  onSecondaryPassphraseChange={setSecondaryPassphrase}
                />
              </TabsContent>

              <TabsContent value="columns" className="mt-0 h-full">
                <ColumnMaskTab
                  sheets={sheets}
                  selectedSheet={selectedSheet}
                  onSelectedSheetChange={setSelectedSheet}
                  currentSheetDef={currentSheetDef}
                  checkedColIndices={checkedColIndices}
                  onToggleColumnChecked={toggleColumnChecked}
                  columnRules={columnRules.filter(
                    (r) => r.sheet === selectedSheet
                  )}
                  onUpdateColumnRule={updateColumnRule}
                />
              </TabsContent>

              <TabsContent value="cells" className="mt-0 h-full">
                <CellOverrideTab
                  overrideCellInput={overrideCellInput}
                  onOverrideCellInputChange={setOverrideCellInput}
                  overrideStrategy={overrideStrategy}
                  onOverrideStrategyChange={setOverrideStrategy}
                  overrideReplacement={overrideReplacement}
                  onOverrideReplacementChange={setOverrideReplacement}
                  onAdd={addCellOverride}
                  cellOverrides={cellOverrides}
                  onRemove={removeCellOverride}
                  conflictCells={conflictCells}
                />
              </TabsContent>

              <TabsContent value="preview" className="mt-0 h-full">
                <PreviewTab
                  preview={preview}
                  previewLoading={previewLoading}
                  onReload={loadPreview}
                  rulesCount={rulesCount}
                  canConfirmBase={retainChecked && rulesCount > 0}
                  confirmSecondCheck={confirmSecondCheck}
                  onConfirmSecondCheckChange={setConfirmSecondCheck}
                />
              </TabsContent>
            </div>
          </Tabs>
        )}

        <DialogFooter className="shrink-0 gap-2">
          <Button variant="outline" onClick={onCancel}>
            取消
          </Button>
          <Button
            onClick={() => {
              void handleConfirm();
            }}
            disabled={!canConfirm || loading}
          >
            <Check className="w-4 h-4 mr-1" />
            应用并脱敏
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface EncRetainTabProps {
  retainChecked: boolean;
  onRetainChange: (v: boolean) => void;
  keyMode: EncSourceKeyMode;
  onKeyModeChange: (v: EncSourceKeyMode) => void;
  secondaryPassphrase: string;
  onSecondaryPassphraseChange: (v: string) => void;
}

function EncRetainTab({
  retainChecked,
  onRetainChange,
  keyMode,
  onKeyModeChange,
  secondaryPassphrase,
  onSecondaryPassphraseChange,
}: EncRetainTabProps) {
  return (
    <div className="space-y-5">
      <Alert variant="default" className="bg-blue-50 border-blue-200">
        <Info className="w-4 h-4 text-blue-600" />
        <AlertTitle className="text-blue-800">关于保留加密源</AlertTitle>
        <AlertDescription className="text-blue-700 text-sm leading-6">
          {RETAIN_MESSAGES.tab0}
        </AlertDescription>
      </Alert>

      <Card>
        <CardContent className="pt-5 space-y-4">
          <div className="flex items-start gap-3">
            <input
              id="retain-enc-source"
              type="checkbox"
              checked={retainChecked}
              onChange={(e) => onRetainChange(e.target.checked)}
              className="mt-1.5 w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
            />
            <Label
              htmlFor="retain-enc-source"
              className="text-sm leading-6 cursor-pointer"
            >
              <span className="font-medium">
                我已阅读三提示并确认：需要反脱敏就必须保留加密源。
              </span>
              <span className="block text-xs text-gray-500 mt-1">
                不勾选将无法自动还原原始 xlsx 样式和格式；后续反脱敏需走路径 B 并验证原件 sha。
              </span>
            </Label>
          </div>

          <div className="border-t pt-4 space-y-3">
            <Label className="text-sm font-medium flex items-center gap-1.5">
              <Key className="w-4 h-4" />
              密钥派生模式
            </Label>

            <div className="space-y-2">
              <label
                className={cn(
                  "flex items-start gap-3 rounded-lg border p-3 cursor-pointer transition-colors",
                  keyMode === "SANDBOX_REUSED"
                    ? "border-blue-400 bg-blue-50"
                    : "border-gray-200 hover:bg-gray-50"
                )}
              >
                <input
                  type="radio"
                  name="key-mode"
                  checked={keyMode === "SANDBOX_REUSED"}
                  onChange={() => onKeyModeChange("SANDBOX_REUSED")}
                  className="mt-0.5"
                />
                <div className="flex-1">
                  <div className="text-sm font-medium">复用沙箱口令（推荐）</div>
                  <div className="text-xs text-gray-500 mt-0.5">
                    直接使用沙箱默认加密口令（fileStore.passphrase），减少记忆负担。
                  </div>
                </div>
              </label>

              <label
                className={cn(
                  "flex items-start gap-3 rounded-lg border p-3 cursor-pointer transition-colors",
                  keyMode === "SECONDARY_PASSPHRASE"
                    ? "border-blue-400 bg-blue-50"
                    : "border-gray-200 hover:bg-gray-50"
                )}
              >
                <input
                  type="radio"
                  name="key-mode"
                  checked={keyMode === "SECONDARY_PASSPHRASE"}
                  onChange={() => onKeyModeChange("SECONDARY_PASSPHRASE")}
                  className="mt-0.5"
                />
                <div className="flex-1 space-y-2">
                  <div className="text-sm font-medium">独立二级口令</div>
                  <div className="text-xs text-gray-500">
                    ecmap 和 encrypted_source 使用独立口令，跨用户分享时更安全。
                  </div>
                  {keyMode === "SECONDARY_PASSPHRASE" && (
                    <PassphraseBox
                      value={secondaryPassphrase}
                      onChange={onSecondaryPassphraseChange}
                      label="二级口令"
                    />
                  )}
                </div>
              </label>

              <label
                className={cn(
                  "flex items-start gap-3 rounded-lg border p-3 cursor-pointer transition-colors",
                  keyMode === "DEVICE_KEY"
                    ? "border-blue-400 bg-blue-50"
                    : "border-gray-200 hover:bg-gray-50"
                )}
              >
                <input
                  type="radio"
                  name="key-mode"
                  checked={keyMode === "DEVICE_KEY"}
                  onChange={() => onKeyModeChange("DEVICE_KEY")}
                  className="mt-0.5"
                />
                <div className="flex-1">
                  <div className="text-sm font-medium">设备绑定密钥</div>
                  <div className="text-xs text-gray-500 mt-0.5">
                    与本机硬件指纹绑定，跨设备反脱敏会直接失败，适用单机严格场景。
                  </div>
                </div>
              </label>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

interface ColumnMaskTabProps {
  sheets: SheetDef[];
  selectedSheet: string;
  onSelectedSheetChange: (n: string) => void;
  currentSheetDef: SheetDef | undefined;
  checkedColIndices: Set<number>;
  onToggleColumnChecked: (colIndex: number, headerText: string) => void;
  columnRules: ColumnMaskRule[];
  onUpdateColumnRule: (
    colIndex: number,
    patch: Partial<Pick<ColumnMaskRule, "strategy" | "replacement">>
  ) => void;
}

function ColumnMaskTab({
  sheets,
  selectedSheet,
  onSelectedSheetChange,
  currentSheetDef,
  checkedColIndices,
  onToggleColumnChecked,
  columnRules,
  onUpdateColumnRule,
}: ColumnMaskTabProps) {
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <Label className="text-sm font-medium">Sheet 选择</Label>
        <Select value={selectedSheet} onValueChange={onSelectedSheetChange}>
          <SelectTrigger className="w-56">
            <Table2 className="w-4 h-4 mr-2 opacity-60" />
            <SelectValue placeholder="选择 Sheet" />
          </SelectTrigger>
          <SelectContent>
            {sheets.map((s) => (
              <SelectItem key={s.name} value={s.name}>
                {s.name}（{s.max_row} 行 / {s.max_col} 列）
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {!currentSheetDef ? (
        <Alert>
          <AlertTriangle className="w-4 h-4" />
          <AlertTitle>没有可用的 Sheet</AlertTitle>
          <AlertDescription>
            当前文件结构未成功解析，请关闭对话框后重试。
          </AlertDescription>
        </Alert>
      ) : (
        <div className="border rounded-lg overflow-hidden">
          <Table>
            <TableHeader className="bg-gray-50">
              <TableRow>
                <TableHead className="w-12 text-center">脱敏</TableHead>
                <TableHead className="w-24">列号</TableHead>
                <TableHead>表头</TableHead>
                <TableHead className="w-48">示例值</TableHead>
                <TableHead className="w-64">脱敏策略</TableHead>
                <TableHead className="w-44">替换默认值</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {currentSheetDef.headers.map((header, idx) => {
                const checked = checkedColIndices.has(idx);
                const rule = columnRules.find((r) => r.colIndex === idx);
                const strategy = rule?.strategy ?? "FULL_MASK";
                const isPlaceholder = PLACEHOLDER_STRATEGIES.includes(strategy);
                return (
                  <TableRow key={idx}>
                    <TableCell className="text-center">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => onToggleColumnChecked(idx, header)}
                        className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
                      />
                    </TableCell>
                    <TableCell className="text-xs text-gray-500 font-mono">
                      {String.fromCharCode(65 + idx)}
                    </TableCell>
                    <TableCell className="font-medium text-sm">
                      {header || <span className="text-gray-400">(空)</span>}
                    </TableCell>
                    <TableCell className="text-xs text-gray-500 truncate max-w-xs">
                      {currentSheetDef.data_hint[idx] ?? "—"}
                    </TableCell>
                    <TableCell>
                      <Select
                        disabled={!checked}
                        value={strategy}
                        onValueChange={(v) =>
                          onUpdateColumnRule(idx, {
                            strategy: v as MaskingStrategyId,
                          })
                        }
                      >
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {ACTIVE_STRATEGIES.map((sid) => (
                            <SelectItem key={sid} value={sid}>
                              {STRATEGY_LABELS[sid]}
                            </SelectItem>
                          ))}
                          <div className="px-2 py-1 text-[11px] text-gray-400">
                            ——— DSG 预留占位 ———
                          </div>
                          {PLACEHOLDER_STRATEGIES.map((sid) => (
                            <SelectItem key={sid} value={sid}>
                              {STRATEGY_LABELS[sid]}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      {checked && isPlaceholder && (
                        <p className="mt-1 text-[11px] text-amber-600">
                          预留占位策略：当前按 DEFAULT_VALUE 或 CLEAR 降级处理
                        </p>
                      )}
                    </TableCell>
                    <TableCell>
                      <Input
                        disabled={!checked}
                        value={rule?.replacement ?? ""}
                        onChange={(e) =>
                          onUpdateColumnRule(idx, {
                            replacement: e.target.value || undefined,
                          })
                        }
                        placeholder="策略需要时填入"
                        className="h-8 text-xs"
                      />
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

interface CellOverrideTabProps {
  overrideCellInput: string;
  onOverrideCellInputChange: (v: string) => void;
  overrideStrategy: MaskingStrategyId;
  onOverrideStrategyChange: (v: MaskingStrategyId) => void;
  overrideReplacement: string;
  onOverrideReplacementChange: (v: string) => void;
  onAdd: () => void;
  cellOverrides: CellOverrideRule[];
  onRemove: (idx: number) => void;
  conflictCells: Set<string>;
}

function CellOverrideTab({
  overrideCellInput,
  onOverrideCellInputChange,
  overrideStrategy,
  onOverrideStrategyChange,
  overrideReplacement,
  onOverrideReplacementChange,
  onAdd,
  cellOverrides,
  onRemove,
  conflictCells,
}: CellOverrideTabProps) {
  return (
    <div className="space-y-4">
      <Card>
        <CardContent className="pt-5 space-y-3">
          <Label className="text-sm font-medium flex items-center gap-1.5">
            <Grid3X3 className="w-4 h-4" />
            新增单元格覆盖（支持 A1 或 Sheet1!A1:B3）
          </Label>
          <div className="grid grid-cols-1 md:grid-cols-12 gap-3">
            <div className="md:col-span-4">
              <Input
                value={overrideCellInput}
                onChange={(e) => onOverrideCellInputChange(e.target.value)}
                placeholder="例：Sheet1!A2 或 B3:D5"
              />
            </div>
            <div className="md:col-span-4">
              <Select
                value={overrideStrategy}
                onValueChange={(v) =>
                  onOverrideStrategyChange(v as MaskingStrategyId)
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ACTIVE_STRATEGIES.map((sid) => (
                    <SelectItem key={sid} value={sid}>
                      {STRATEGY_LABELS[sid]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="md:col-span-3">
              <Input
                value={overrideReplacement}
                onChange={(e) => onOverrideReplacementChange(e.target.value)}
                placeholder="替换默认值（可选）"
              />
            </div>
            <div className="md:col-span-1">
              <Button onClick={onAdd} className="w-full">
                <Sparkles className="w-4 h-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {conflictCells.size > 0 && (
        <Alert variant="default" className="bg-amber-50 border-amber-200">
          <AlertTriangle className="w-4 h-4 text-amber-600" />
          <AlertTitle className="text-amber-800">检测到冲突</AlertTitle>
          <AlertDescription className="text-amber-700 text-xs">
            以下单元格同时被列策略与单元格覆盖命中：
            <span className="font-mono ml-1">
              {[...conflictCells].join("、")}
            </span>
            。单元格覆盖优先级更高。
          </AlertDescription>
        </Alert>
      )}

      {cellOverrides.length === 0 ? (
        <div className="text-sm text-gray-400 text-center py-8">
          暂无单元格覆盖规则
        </div>
      ) : (
        <div className="border rounded-lg overflow-hidden">
          <Table>
            <TableHeader className="bg-gray-50">
              <TableRow>
                <TableHead>位置</TableHead>
                <TableHead>策略</TableHead>
                <TableHead>替换值</TableHead>
                <TableHead className="w-16" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {cellOverrides.map((o, idx) => {
                const colLetter = String.fromCharCode(65 + o.col);
                const key = `${o.sheet}!${o.row}:${o.col}`;
                const conflict = conflictCells.has(key);
                return (
                  <TableRow
                    key={idx}
                    className={conflict ? "bg-amber-50" : undefined}
                  >
                    <TableCell>
                      <span className="font-mono text-xs">
                        {o.sheet}!{colLetter}
                        {o.row + 1}
                      </span>
                      {conflict && (
                        <span className="ml-2 text-[10px] text-amber-600">
                          冲突
                        </span>
                      )}
                    </TableCell>
                    <TableCell className="text-xs">
                      {STRATEGY_LABELS[o.strategy]}
                    </TableCell>
                    <TableCell className="text-xs text-gray-600">
                      {o.replacement ?? "—"}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        size="icon"
                        variant="ghost"
                        onClick={() => onRemove(idx)}
                      >
                        <X className="w-4 h-4" />
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

interface PreviewTabProps {
  preview: ExcelMaskPreview | null;
  previewLoading: boolean;
  onReload: () => void;
  rulesCount: number;
  canConfirmBase: boolean;
  confirmSecondCheck: boolean;
  onConfirmSecondCheckChange: (v: boolean) => void;
}

function PreviewTab({
  preview,
  previewLoading,
  onReload,
  rulesCount,
  canConfirmBase,
  confirmSecondCheck,
  onConfirmSecondCheckChange,
}: PreviewTabProps) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="text-sm text-gray-600">
          当前共 <b>{rulesCount}</b> 条规则；仅展示前 20 行差异作为预览。
        </div>
        <Button size="sm" variant="outline" onClick={onReload}>
          <RefreshCw
            className={cn("w-4 h-4 mr-1", previewLoading && "animate-spin")}
          />
          刷新预览
        </Button>
      </div>

      {!canConfirmBase && (
        <Alert variant="default" className="bg-amber-50 border-amber-200">
          <AlertTriangle className="w-4 h-4 text-amber-600" />
          <AlertTitle className="text-amber-800">还不能应用</AlertTitle>
          <AlertDescription className="text-amber-700 text-sm">
            请先回到「加密源 &amp; 密钥」Tab 勾选保留加密源确认，并至少添加 1 条规则。
          </AlertDescription>
        </Alert>
      )}

      {canConfirmBase && (
        <Card>
          <CardContent className="pt-4">
            <label className="flex items-start gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={confirmSecondCheck}
                onChange={(e) =>
                  onConfirmSecondCheckChange(e.target.checked)
                }
                className="mt-1 w-4 h-4 text-amber-600 border-gray-300 rounded focus:ring-amber-500"
              />
              <div className="text-sm leading-6">
                <div className="font-medium text-amber-800">
                  最终确认：我已知晓三提示并同意执行
                </div>
                <div className="text-xs text-amber-700 mt-0.5">
                  {RETAIN_MESSAGES.confirm}
                </div>
              </div>
            </label>
          </CardContent>
        </Card>
      )}

      {previewLoading ? (
        <div className="py-10 text-center text-sm text-gray-500">
          正在生成预览...
        </div>
      ) : !preview ? (
        <div className="py-10 text-center text-sm text-gray-400 border border-dashed rounded-lg">
          点击上方「刷新预览」查看前 20 行脱敏效果
        </div>
      ) : (
        <div className="space-y-3">
          {preview.conflicts.length > 0 && (
            <Alert variant="default" className="bg-amber-50 border-amber-200">
              <AlertTriangle className="w-4 h-4 text-amber-600" />
              <AlertTitle className="text-amber-800">
                预览提示（{preview.conflicts.length}）
              </AlertTitle>
              <AlertDescription className="text-amber-700 text-xs">
                {preview.conflicts.slice(0, 5).join("；")}
                {preview.conflicts.length > 5 &&
                  ` 等 ${preview.conflicts.length} 项`}
              </AlertDescription>
            </Alert>
          )}

          <div className="border rounded-lg overflow-hidden max-h-[50vh] overflow-auto">
            <Table>
              <TableHeader className="bg-gray-50 sticky top-0">
                <TableRow>
                  <TableHead className="w-16 text-xs">行#</TableHead>
                  <TableHead className="w-24 text-xs">Sheet</TableHead>
                  <TableHead className="text-xs">原值（前 5 列抽样）</TableHead>
                  <TableHead className="text-xs">脱敏后</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {preview.preview_rows.map((row, idx) => (
                  <TableRow key={idx}>
                    <TableCell className="text-xs text-gray-500 font-mono">
                      {row.row_index}
                    </TableCell>
                    <TableCell className="text-xs text-gray-500">
                      {row.sheet}
                    </TableCell>
                    <TableCell className="text-xs">
                      <div className="flex flex-wrap gap-1">
                        {row.original_preview.slice(0, 5).map((v, i) => (
                          <span
                            key={i}
                            className="inline-block px-1.5 py-0.5 bg-gray-100 text-gray-500 rounded truncate max-w-[140px]"
                          >
                            {v ?? "∅"}
                          </span>
                        ))}
                      </div>
                    </TableCell>
                    <TableCell className="text-xs">
                      <div className="flex flex-wrap gap-1">
                        {row.masked.slice(0, 5).map((v, i) => {
                          const orig = row.original_preview[i];
                          const changed = v !== orig;
                          return (
                            <span
                              key={i}
                              className={cn(
                                "inline-block px-1.5 py-0.5 rounded truncate max-w-[140px]",
                                changed
                                  ? "bg-red-50 text-red-700 font-semibold border border-red-200"
                                  : "bg-gray-50 text-gray-500"
                              )}
                            >
                              {v}
                            </span>
                          );
                        })}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </div>
      )}
    </div>
  );
}
