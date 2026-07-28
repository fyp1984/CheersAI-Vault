export type EnterpriseInputFormat = "text" | "markdown" | "csv" | "excel" | "docx" | "pdf" | "powerpoint";

type FormatDefinition = {
  extension: `.${string}`;
  inputFormat: EnterpriseInputFormat;
  label: string;
  mimeTypes: readonly string[];
};

/**
 * The web-facing projection of the enterprise format boundary.
 * Keep this list aligned with engine-core::FormatCatalog; unsupported future
 * format families intentionally do not appear as selectable enterprise input.
 */
export const formatCatalog: readonly FormatDefinition[] = [
  { extension: ".txt", inputFormat: "text", label: "TXT", mimeTypes: ["text/plain"] },
  {
    extension: ".md",
    inputFormat: "markdown",
    label: "Markdown",
    mimeTypes: ["text/markdown", "text/plain"],
  },
  {
    extension: ".markdown",
    inputFormat: "markdown",
    label: "Markdown",
    mimeTypes: ["text/markdown", "text/plain"],
  },
  {
    extension: ".csv",
    inputFormat: "csv",
    label: "CSV",
    mimeTypes: ["text/csv", "application/csv", "text/plain"],
  },
  {
    extension: ".xls",
    inputFormat: "excel",
    label: "Excel",
    mimeTypes: [
      "application/vnd.ms-excel",
      "application/octet-stream",
    ],
  },
  {
    extension: ".xlsx",
    inputFormat: "excel",
    label: "Excel",
    mimeTypes: [
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
      "application/octet-stream",
    ],
  },
  {
    extension: ".docx",
    inputFormat: "docx",
    label: "DOCX",
    mimeTypes: ["application/vnd.openxmlformats-officedocument.wordprocessingml.document"],
  },
  { extension: ".pdf", inputFormat: "pdf", label: "PDF", mimeTypes: ["application/pdf"] },
  {
    extension: ".ppt",
    inputFormat: "powerpoint",
    label: "PPT",
    mimeTypes: [
      "application/vnd.ms-powerpoint",
      "application/octet-stream",
    ],
  },
  {
    extension: ".pptx",
    inputFormat: "powerpoint",
    label: "PPTX",
    mimeTypes: [
      "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ],
  },
];

export const allowedExtensions = formatCatalog.map(({ extension }) => extension);

export const accept = formatCatalog
  .flatMap(({ extension, mimeTypes }) => [extension, ...mimeTypes])
  .join(",");

function basename(value: string): string {
  return value.replace(/^.*[\\/]/, "");
}

function extensionOf(value: string): string | null {
  const name = basename(value);
  const separator = name.lastIndexOf(".");
  return separator > 0 ? name.slice(separator).toLowerCase() : null;
}

export function isSupported(file: File): boolean {
  const extension = extensionOf(file.name);
  return extension !== null && formatCatalog.some((definition) => definition.extension === extension);
}

export function inputFormatFromFilename(filename: string): EnterpriseInputFormat | null {
  const extension = extensionOf(filename);
  return formatCatalog.find((definition) => definition.extension === extension)?.inputFormat ?? null;
}

export function formatLabel(inputFormat: string): string {
  return formatCatalog.find((definition) => definition.inputFormat === inputFormat)?.label ?? inputFormat;
}

export function maskedArtifactFilename(displayName: string): string {
  const name = basename(displayName);
  const separator = name.lastIndexOf(".");
  const stem = separator > 0 ? name.slice(0, separator) : name;
  return `${stem}.masked.md`;
}
