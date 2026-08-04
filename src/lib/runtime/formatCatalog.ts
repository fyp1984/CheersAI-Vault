/**
 * 浏览器多文件脱敏链路使用的格式白名单投影。
 *
 * 允许格式必须与当前企业 Runtime 一致——本清单逐项镜像 Runtime/
 * engine-core 格式边界在 Web 侧的既有投影，不新增、不放宽任何格式。
 * 前端校验只是提示，Runtime 仍是最终安全门禁，不因这里的判断而跳过
 * 服务端校验。
 */

export type RuntimeInputFormat = "text" | "markdown" | "csv" | "excel" | "docx" | "pdf" | "powerpoint";

interface RuntimeFormatDefinition {
  extension: `.${string}`;
  inputFormat: RuntimeInputFormat;
  label: string;
}

export const runtimeFormatCatalog: readonly RuntimeFormatDefinition[] = [
  { extension: ".txt", inputFormat: "text", label: "TXT" },
  { extension: ".md", inputFormat: "markdown", label: "Markdown" },
  { extension: ".markdown", inputFormat: "markdown", label: "Markdown" },
  { extension: ".csv", inputFormat: "csv", label: "CSV" },
  { extension: ".xls", inputFormat: "excel", label: "Excel" },
  { extension: ".xlsx", inputFormat: "excel", label: "Excel" },
  { extension: ".docx", inputFormat: "docx", label: "DOCX" },
  { extension: ".pdf", inputFormat: "pdf", label: "PDF" },
  { extension: ".ppt", inputFormat: "powerpoint", label: "PPT" },
  { extension: ".pptx", inputFormat: "powerpoint", label: "PPTX" },
];

export const runtimeAllowedExtensions = runtimeFormatCatalog.map(({ extension }) => extension);

export const runtimeAcceptAttribute = runtimeAllowedExtensions.join(",");

function basename(value: string): string {
  return value.replace(/^.*[\\/]/, "");
}

function extensionOf(filename: string): string | null {
  const name = basename(filename);
  const separator = name.lastIndexOf(".");
  return separator > 0 ? name.slice(separator).toLowerCase() : null;
}

export function isRuntimeFormatSupported(filename: string): boolean {
  const extension = extensionOf(filename);
  return extension !== null && runtimeFormatCatalog.some((definition) => definition.extension === extension);
}

export function runtimeInputFormatFromFilename(filename: string): RuntimeInputFormat | null {
  const extension = extensionOf(filename);
  return runtimeFormatCatalog.find((definition) => definition.extension === extension)?.inputFormat ?? null;
}

export function runtimeFormatLabel(inputFormat: string): string {
  return runtimeFormatCatalog.find((definition) => definition.inputFormat === inputFormat)?.label ?? inputFormat;
}
