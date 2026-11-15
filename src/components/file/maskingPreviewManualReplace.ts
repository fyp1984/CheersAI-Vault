import type { MappingEntry, PreviewResult } from "@/types/commands";

export interface PreviewFile {
  fileName: string;
  preview: PreviewResult;
}

function sanitizeMaskedFileStem(fileStem: string) {
  const sanitized = fileStem
    .replace(/[<>:"/\\|?\x00-\x1f]/g, "_")
    .replace(/^[ .]+|[ .]+$/g, "");

  return sanitized || "masked_file";
}

function replaceLiteral(source: string, search: string, replacement: string) {
  return source.split(search).join(replacement);
}

function countLiteralMatches(source: string, search: string) {
  return source.split(search).length - 1;
}

function rebuildMarkdownFromRows(rows: string[][]) {
  return rows.map((row) => row.join(", ")).join("\n");
}

function updateMappingEntries(
  mapping: MappingEntry[] | undefined,
  findText: string,
  replaceText: string,
) {
  if (!mapping) {
    return mapping;
  }

  return mapping.map((entry) => {
    if (entry.masked.includes(findText)) {
      return {
        ...entry,
        masked: replaceLiteral(entry.masked, findText, replaceText),
      };
    }

    if (entry.original.includes(findText)) {
      return {
        ...entry,
        masked: replaceLiteral(entry.original, findText, replaceText),
      };
    }

    return entry;
  });
}

function updateMaskedFileFields(
  preview: PreviewResult,
  previousMapping: MappingEntry[] | undefined,
  nextMapping: MappingEntry[] | undefined,
  findText: string,
  replaceText: string,
) {
  let nextStem = preview.masked_file_stem;
  let filenameMatchCount = 0;

  if (nextStem.includes(findText)) {
    filenameMatchCount += countLiteralMatches(nextStem, findText);
    nextStem = replaceLiteral(nextStem, findText, replaceText);
  } else if (previousMapping && nextMapping) {
    for (let index = 0; index < previousMapping.length; index += 1) {
      const previousEntry = previousMapping[index];
      const nextEntry = nextMapping[index];
      if (!previousEntry || !nextEntry) {
        continue;
      }
      if (!previousEntry.original.includes(findText) || !nextStem.includes(previousEntry.masked)) {
        continue;
      }
      filenameMatchCount += countLiteralMatches(previousEntry.original, findText);
      nextStem = replaceLiteral(nextStem, previousEntry.masked, nextEntry.masked);
    }
  }

  const sanitizedStem = sanitizeMaskedFileStem(nextStem);
  const dotIndex = preview.original_file_name.lastIndexOf(".");
  const extension = dotIndex > 0 ? preview.original_file_name.slice(dotIndex + 1) : "";

  return {
    masked_file_stem: sanitizedStem,
    masked_file_name: extension ? `${sanitizedStem}.${extension}` : sanitizedStem,
    filenameMatchCount,
  };
}

export function applyManualReplacementToPreview(
  preview: PreviewResult,
  findText: string,
  replaceText: string,
) {
  if (!findText.trim()) {
    return { preview, count: 0 };
  }

  let count = 0;
  let rebuiltFromOriginal = false;

  const maskedRows = preview.masked_rows.map((row, rowIdx) => {
    const originalRow = preview.original_rows[rowIdx] ?? [];

    return row.map((cell, cellIdx) => {
      const originalCell = originalRow[cellIdx] ?? "";

      if (cell.includes(findText)) {
        count += countLiteralMatches(cell, findText);
        return replaceLiteral(cell, findText, replaceText);
      }

      if (originalCell.includes(findText)) {
        count += countLiteralMatches(originalCell, findText);
        rebuiltFromOriginal = true;
        return replaceLiteral(originalCell, findText, replaceText);
      }

      return cell;
    });
  });
  const nextMapping = updateMappingEntries(preview.mapping, findText, replaceText);
  const nextFileFields = updateMaskedFileFields(
    preview,
    preview.mapping,
    nextMapping,
    findText,
    replaceText,
  );
  const totalCount = count + nextFileFields.filenameMatchCount;

  if (totalCount === 0) {
    return { preview, count: 0 };
  }

  const maskedMarkdown = preview.masked_markdown == null
    ? preview.masked_markdown
    : count === 0
      ? preview.masked_markdown
      : rebuiltFromOriginal || !preview.masked_markdown.includes(findText)
        ? rebuildMarkdownFromRows(maskedRows)
        : replaceLiteral(preview.masked_markdown, findText, replaceText);

  return {
    count: totalCount,
    preview: {
      ...preview,
      masked_rows: maskedRows,
      masked_markdown: maskedMarkdown,
      mapping: nextMapping,
      masked_file_stem: nextFileFields.masked_file_stem,
      masked_file_name: nextFileFields.masked_file_name,
    },
  };
}

export function applyManualReplacementToPreviewFiles(
  previews: PreviewFile[],
  currentFileIndex: number,
  findText: string,
  replaceText: string,
) {
  if (!findText.trim()) {
    return { previews, count: 0 };
  }

  let count = 0;
  const nextPreviews = previews.map((filePreview, index) => {
    if (index !== currentFileIndex) {
      return filePreview;
    }

    const result = applyManualReplacementToPreview(filePreview.preview, findText, replaceText);
    count = result.count;
    return {
      ...filePreview,
      fileName: result.preview.masked_file_name,
      preview: result.preview,
    };
  });

  return {
    previews: nextPreviews,
    count,
  };
}
