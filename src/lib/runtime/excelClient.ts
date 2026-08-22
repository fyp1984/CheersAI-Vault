import type {
  ExcelMaskingConfig,
  ExcelMaskPreview,
  SheetDef,
} from "@/types/commands";
import type { RuntimeExcelPersistArtifactsResponse } from "@/types/runtime";
import {
  classifyRuntimeFetchError,
  classifyRuntimeHttpResponse,
  parseRuntimeJsonResponse,
} from "./errorClassification";
import { runtimeBaseUrl } from "./client";

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
