import { Fragment, type ReactNode } from "react";
import { cn } from "@/lib/utils";

interface HeadingItem {
  id: string;
  text: string;
  level: number;
}

type Block =
  | { type: "heading"; level: number; text: string; id: string }
  | { type: "paragraph"; text: string }
  | { type: "list"; ordered: boolean; items: string[] }
  | { type: "blockquote"; lines: string[] }
  | { type: "code"; language: string; code: string }
  | { type: "table"; headers: string[]; rows: string[][] }
  | { type: "hr" };

function slugifyHeading(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[【】[\]()]/g, "")
    .replace(/[^\p{Letter}\p{Number}\s-]/gu, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

function splitTableRow(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

function isTableSeparator(line: string): boolean {
  const normalized = line.trim().replace(/\|/g, "").replace(/:/g, "").replace(/-/g, "");
  return normalized.length === 0 && line.includes("-");
}

function startsWithBlockToken(line: string): boolean {
  const trimmed = line.trim();
  return (
    trimmed.startsWith("#") ||
    trimmed.startsWith(">") ||
    trimmed.startsWith("```") ||
    trimmed.startsWith("- ") ||
    /^\d+\.\s+/.test(trimmed) ||
    trimmed === "---" ||
    /^<a id=".*"><\/a>$/.test(trimmed)
  );
}

function extractBlocks(markdown: string): { blocks: Block[]; headings: HeadingItem[] } {
  const lines = markdown.replace(/\r\n/g, "\n").split("\n");
  const blocks: Block[] = [];
  const headings: HeadingItem[] = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const trimmed = line.trim();

    if (!trimmed) {
      continue;
    }

    if (/^<a id=".*"><\/a>$/.test(trimmed)) {
      continue;
    }

    if (trimmed.startsWith("```")) {
      const language = trimmed.slice(3).trim();
      const codeLines: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index].trim().startsWith("```")) {
        codeLines.push(lines[index]);
        index += 1;
      }
      blocks.push({ type: "code", language, code: codeLines.join("\n") });
      continue;
    }

    const headingMatch = /^(#{1,6})\s+(.*)$/.exec(trimmed);
    if (headingMatch) {
      const level = headingMatch[1].length;
      const text = headingMatch[2].trim();
      const id = slugifyHeading(text);
      blocks.push({ type: "heading", level, text, id });
      if (level >= 2 && level <= 3) {
        headings.push({ id, text, level });
      }
      continue;
    }

    if (trimmed === "---") {
      blocks.push({ type: "hr" });
      continue;
    }

    if (trimmed.startsWith(">")) {
      const quoteLines: string[] = [trimmed.replace(/^>\s?/, "")];
      while (index + 1 < lines.length && lines[index + 1].trim().startsWith(">")) {
        index += 1;
        quoteLines.push(lines[index].trim().replace(/^>\s?/, ""));
      }
      blocks.push({ type: "blockquote", lines: quoteLines });
      continue;
    }

    const nextLine = lines[index + 1]?.trim() ?? "";
    if (trimmed.includes("|") && nextLine && isTableSeparator(nextLine)) {
      const headers = splitTableRow(trimmed);
      const rows: string[][] = [];
      index += 2;
      while (index < lines.length && lines[index].includes("|")) {
        rows.push(splitTableRow(lines[index]));
        index += 1;
      }
      index -= 1;
      blocks.push({ type: "table", headers, rows });
      continue;
    }

    if (trimmed.startsWith("- ") || /^\d+\.\s+/.test(trimmed)) {
      const ordered = /^\d+\.\s+/.test(trimmed);
      const items: string[] = [trimmed.replace(ordered ? /^\d+\.\s+/ : /^-\s+/, "")];
      while (index + 1 < lines.length) {
        const next = lines[index + 1].trim();
        const matchesOrdered = /^\d+\.\s+/.test(next);
        const matchesUnordered = next.startsWith("- ");
        if ((ordered && matchesOrdered) || (!ordered && matchesUnordered)) {
          index += 1;
          items.push(lines[index].trim().replace(ordered ? /^\d+\.\s+/ : /^-\s+/, ""));
          continue;
        }
        break;
      }
      blocks.push({ type: "list", ordered, items });
      continue;
    }

    const paragraphLines = [trimmed];
    while (index + 1 < lines.length) {
      const next = lines[index + 1].trim();
      if (!next || startsWithBlockToken(next)) {
        break;
      }
      if (next.includes("|") && isTableSeparator(lines[index + 2]?.trim() ?? "")) {
        break;
      }
      index += 1;
      paragraphLines.push(lines[index].trim());
    }
    blocks.push({ type: "paragraph", text: paragraphLines.join(" ") });
  }

  return { blocks, headings };
}

function renderInline(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let remaining = text;
  let key = 0;

  while (remaining.length > 0) {
    const tokenMatch = remaining.match(/(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\([^)]+\))/);
    if (!tokenMatch || tokenMatch.index === undefined) {
      nodes.push(<Fragment key={`text-${key}`}>{remaining}</Fragment>);
      break;
    }

    const { index } = tokenMatch;
    const token = tokenMatch[0];

    if (index > 0) {
      nodes.push(<Fragment key={`text-${key}`}>{remaining.slice(0, index)}</Fragment>);
      key += 1;
    }

    if (token.startsWith("`")) {
      nodes.push(
        <code
          key={`code-${key}`}
          className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[0.92em] text-slate-900"
        >
          {token.slice(1, -1)}
        </code>
      );
    } else if (token.startsWith("**")) {
      nodes.push(
        <strong key={`strong-${key}`} className="font-semibold text-slate-950">
          {token.slice(2, -2)}
        </strong>
      );
    } else {
      const linkMatch = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(token);
      if (linkMatch) {
        const [, label, href] = linkMatch;
        nodes.push(
          <a
            key={`link-${key}`}
            href={href}
            target={href.startsWith("http") ? "_blank" : undefined}
            rel={href.startsWith("http") ? "noreferrer" : undefined}
            className="text-blue-600 underline underline-offset-4 hover:text-blue-700"
          >
            {label}
          </a>
        );
      }
    }

    remaining = remaining.slice(index + token.length);
    key += 1;
  }

  return nodes;
}

export function extractDocumentHeadings(markdown: string): HeadingItem[] {
  return extractBlocks(markdown).headings;
}

interface MarkdownArticleProps {
  markdown: string;
  className?: string;
}

export function MarkdownArticle({ markdown, className }: MarkdownArticleProps) {
  const { blocks } = extractBlocks(markdown);

  return (
    <article className={cn("space-y-6 text-slate-700", className)}>
      {blocks.map((block, index) => {
        if (block.type === "heading") {
          if (block.level === 1) {
            return (
              <h1 key={block.id} id={block.id} className="text-3xl font-semibold tracking-tight text-slate-950">
                {block.text}
              </h1>
            );
          }

          if (block.level === 2) {
            return (
              <h2
                key={block.id}
                id={block.id}
                className="scroll-mt-24 border-b border-slate-200 pb-2 text-2xl font-semibold text-slate-950"
              >
                {block.text}
              </h2>
            );
          }

          return (
            <h3
              key={block.id}
              id={block.id}
              className="scroll-mt-24 text-lg font-semibold text-slate-900"
            >
              {block.text}
            </h3>
          );
        }

        if (block.type === "paragraph") {
          return (
            <p key={`paragraph-${index}`} className="text-sm leading-7 text-slate-600 sm:text-[15px]">
              {renderInline(block.text)}
            </p>
          );
        }

        if (block.type === "blockquote") {
          return (
            <div
              key={`quote-${index}`}
              className="rounded-2xl border border-blue-100 bg-blue-50/80 px-4 py-3 text-sm leading-7 text-blue-900"
            >
              {block.lines.map((line, lineIndex) => (
                <p key={`quote-line-${lineIndex}`}>{renderInline(line)}</p>
              ))}
            </div>
          );
        }

        if (block.type === "list") {
          const ListTag = block.ordered ? "ol" : "ul";
          return (
            <ListTag
              key={`list-${index}`}
              className={cn(
                "space-y-2 pl-5 text-sm leading-7 text-slate-600",
                block.ordered ? "list-decimal" : "list-disc"
              )}
            >
              {block.items.map((item, itemIndex) => (
                <li key={`item-${itemIndex}`}>{renderInline(item)}</li>
              ))}
            </ListTag>
          );
        }

        if (block.type === "code") {
          return (
            <div key={`code-${index}`} className="overflow-hidden rounded-2xl border border-slate-200 bg-slate-950">
              {block.language && (
                <div className="border-b border-slate-800 px-4 py-2 text-xs uppercase tracking-[0.24em] text-slate-400">
                  {block.language}
                </div>
              )}
              <pre className="overflow-x-auto px-4 py-4 text-sm leading-6 text-slate-100">
                <code>{block.code}</code>
              </pre>
            </div>
          );
        }

        if (block.type === "table") {
          return (
            <div key={`table-${index}`} className="overflow-hidden rounded-2xl border border-slate-200">
              <div className="overflow-x-auto">
                <table className="min-w-full divide-y divide-slate-200 text-sm">
                  <thead className="bg-slate-50">
                    <tr>
                      {block.headers.map((header) => (
                        <th key={header} className="px-4 py-3 text-left font-semibold text-slate-900">
                          {renderInline(header)}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100 bg-white">
                    {block.rows.map((row, rowIndex) => (
                      <tr key={`row-${rowIndex}`}>
                        {row.map((cell, cellIndex) => (
                          <td key={`cell-${rowIndex}-${cellIndex}`} className="px-4 py-3 align-top text-slate-600">
                            {renderInline(cell)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          );
        }

        return <hr key={`hr-${index}`} className="border-slate-200" />;
      })}
    </article>
  );
}
