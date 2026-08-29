import type {
  ExcelMaskingConfig,
  ExcelMaskPreview,
  SheetDef,
} from "@/types/commands";
import type {
  RuntimeExcelArtifactMemberKind,
  RuntimeExcelPersistArtifactsResponse,
  RuntimeExcelPersistedFile,
} from "@/types/runtime";
import {
  classifyRuntimeFetchError,
  classifyRuntimeHttpResponse,
  parseRuntimeJsonResponse,
} from "./errorClassification";
import { runtimeBaseUrl } from "./client";

/**
 * 企业 Excel 产物清单驱动的下载动作（R-closeout 工作包 C）。
 *
 * 浏览器结果页只能渲染 Runtime manifest 中真实存在的 Excel 成员；未保留
 * 加密源时绝不显示「下载加密源」动作，也就永远不会发出该成员的下载请求。
 */
export const EXCEL_ARTIFACT_ACTION_LABELS: ReadonlyArray<{
  kind: RuntimeExcelArtifactMemberKind;
  label: string;
}> = [
  { kind: "masked_workbook", label: "下载工作簿" },
  { kind: "report", label: "下载报告" },
  { kind: "ecmap", label: "下载 ECMAP" },
  { kind: "encrypted_source", label: "下载加密源" },
];

export function excelMemberActionsForKinds(
  members: RuntimeExcelPersistedFile[] | "loading" | "error" | undefined
): ReadonlyArray<RuntimeExcelArtifactMemberKind> {
  if (!Array.isArray(members)) return [];
  return EXCEL_ARTIFACT_ACTION_LABELS.filter((action) =>
    members.some((member) => member.kind === action.kind)
  ).map((action) => action.kind);
}

/**
 * 企业 Excel 恢复失败文案（R-closeout 工作包 C）：全部为固定安全文案，
 * 绝不回显口令、路径、堆栈、SQL 或密文正文。
 */
export function excelRestoreErrorMessage(
  reason: string,
  code?: string,
  message?: string
): string {
  if (reason === "network") {
    return "当前连不上本地服务，请确认服务已启动后再试。";
  }
  if (reason === "invalid-count") {
    return "恢复结果异常，没有生成任何文件。请稍后再试。";
  }
  switch (code) {
    case "EXCEL_RESTORE_DECRYPT_FAILED":
      return "恢复失败：口令不正确，或映射文件与口令不匹配。请确认口令后重试。";
    case "EXCEL_RESTORE_MISMATCH":
      return "恢复失败：材料校验未通过（用户原件或加密源与映射记录不一致）。";
    case "EXCEL_RESTORE_MATERIAL_MISSING":
      return "恢复失败：缺少所需材料。请检查恢复路径的选择是否正确。";
    case "EXCEL_RESTORE_MODE_INVALID":
    case "EXCEL_RESTORE_MODE_REQUIRED":
      return "恢复失败：恢复路径参数无效。请刷新后重试。";
    case "NOT_FOUND":
      return "这个处理结果已经不存在，或暂时无法恢复。请返回重新选择。";
    default:
      return message ?? "恢复没有成功，请稍后再试。";
  }
}

export type RuntimeFetchResult<T> =
  | { ok: true; data: T }
  | { ok: false; reason: "network" }
  | {
      ok: false;
      reason: "http";
      status: number;
      code?: string;
      message?: string;
      retryable?: boolean;
    }
  | { ok: false; reason: "parse" };

function buildUrl(path: string): string {
  return `${runtimeBaseUrl}${path}`;
}

async function postExcelJson<T>(
  path: string,
  file: File,
  config?: ExcelMaskingConfig,
  maxRows?: number
): Promise<RuntimeFetchResult<T>> {
  const form = new FormData();
  form.set("file", file, file.name);
  if (config) {
    form.set("config", JSON.stringify(config));
  }
  if (typeof maxRows === "number") {
    form.set("max_rows", String(maxRows));
  }

  let response: Response;
  try {
    response = await fetch(buildUrl(path), {
      method: "POST",
      body: form,
      cache: "no-store",
      credentials: "omit",
    });
  } catch (error) {
    return classifyRuntimeFetchError(error);
  }

  if (!response.ok) {
    return classifyRuntimeHttpResponse(response);
  }
  return parseRuntimeJsonResponse<T>(response);
}

export function parseRuntimeExcelStructure(
  file: File
): Promise<RuntimeFetchResult<SheetDef[]>> {
  return postExcelJson<SheetDef[]>("/api/v1/excel/parse-structure", file);
}

export function previewRuntimeExcelMasking(
  file: File,
  config: ExcelMaskingConfig,
  maxRows = 20
): Promise<RuntimeFetchResult<ExcelMaskPreview>> {
  return postExcelJson<ExcelMaskPreview>(
    "/api/v1/excel/preview",
    file,
    config,
    maxRows
  );
}

export async function persistRuntimeExcelArtifacts(
  file: File,
  config: ExcelMaskingConfig,
  ruleIds: string[]
): Promise<RuntimeFetchResult<RuntimeExcelPersistArtifactsResponse>> {
  const form = new FormData();
  form.set("file", file, file.name);
  form.set("config", JSON.stringify(config));
  form.set("rule_ids", JSON.stringify(ruleIds));

  let response: Response;
  try {
    response = await fetch(buildUrl("/api/v1/excel/jobs"), {
      method: "POST",
      body: form,
      cache: "no-store",
      credentials: "omit",
    });
  } catch (error) {
    return classifyRuntimeFetchError(error);
  }

  if (!response.ok) {
    return classifyRuntimeHttpResponse(response);
  }
  return parseRuntimeJsonResponse<RuntimeExcelPersistArtifactsResponse>(response);
}
