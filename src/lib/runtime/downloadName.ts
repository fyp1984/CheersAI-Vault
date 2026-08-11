export function safeStem(displayName: string): string {
  const withoutPath = displayName.replace(/^.*[\\/]/, "");
  // eslint-disable-next-line no-control-regex
  const withoutControlChars = withoutPath.replace(/[\x00-\x1f\x7f]/g, "");
  const withoutReservedChars = withoutControlChars.replace(/[<>:"/\\|?]/g, "_");
  const dot = withoutReservedChars.lastIndexOf(".");
  const stem = dot > 0 ? withoutReservedChars.slice(0, dot) : withoutReservedChars;
  return stem.trim() || "artifact";
}

export function parseContentDispositionFilename(headerValue: string | null): string | null {
  if (!headerValue) return null;

  const encodedMatch = headerValue.match(/filename\*\s*=\s*UTF-8''([^;]+)/i);
  if (encodedMatch?.[1]) {
    try {
      return decodeURIComponent(encodedMatch[1].trim());
    } catch {
      return null;
    }
  }

  const quotedMatch = headerValue.match(/filename\s*=\s*"([^"]+)"/i);
  if (quotedMatch?.[1]) {
    return quotedMatch[1].trim() || null;
  }

  const plainMatch = headerValue.match(/filename\s*=\s*([^;]+)/i);
  if (plainMatch?.[1]) {
    return plainMatch[1].trim() || null;
  }

  return null;
}

export function artifactDownloadName(displayName: string, headerValue: string | null): string {
  return parseContentDispositionFilename(headerValue) ?? `${safeStem(displayName)}_脱敏.md`;
}

export function restoreDownloadName(displayName: string, headerValue: string | null): string {
  return parseContentDispositionFilename(headerValue) ?? `${safeStem(displayName)}_还原.md`;
}
