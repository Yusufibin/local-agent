/** Markdown is rendered as Svelte elements; model output never becomes raw HTML. */
export type Inline = { kind: "text" | "code" | "strong" | "em" | "link"; text: string; href?: string };
export type Block =
  | { kind: "code"; text: string; language: string }
  | { kind: "heading"; level: number; text: string }
  | { kind: "paragraph" | "quote"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "rule" };

export function safeLink(url: string): string | undefined {
  if (/[\u0000-\u0020\u007f]/.test(url)) return undefined;
  if (url.startsWith("#")) return url;
  try {
    const parsed = new URL(url);
    return ["https:", "http:", "mailto:"].includes(parsed.protocol) ? parsed.href : undefined;
  } catch { return undefined; }
}

export function inline(text: string): Inline[] {
  const tokens: Inline[] = [];
  const pattern = /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\n]+\*|\[[^\]\n]+\]\([^\s)]+\))/g;
  let cursor = 0;
  for (const match of text.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > cursor) tokens.push({ kind: "text", text: text.slice(cursor, index) });
    const value = match[0];
    if (value.startsWith("`")) tokens.push({ kind: "code", text: value.slice(1, -1) });
    else if (value.startsWith("**")) tokens.push({ kind: "strong", text: value.slice(2, -2) });
    else if (value.startsWith("*")) tokens.push({ kind: "em", text: value.slice(1, -1) });
    else {
      const parts = /^\[([^\]]+)\]\((.+)\)$/.exec(value);
      const href = parts ? safeLink(parts[2]) : undefined;
      tokens.push(href ? { kind: "link", text: parts![1], href } : { kind: "text", text: value });
    }
    cursor = index + value.length;
  }
  if (cursor < text.length) tokens.push({ kind: "text", text: text.slice(cursor) });
  return tokens;
}

export function markdown(text: string): Block[] {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  const blocks: Block[] = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index];
    if (!line.trim()) { index++; continue; }
    const fence = /^\s*(`{3,}|~{3,})([^\s]*)/.exec(line);
    if (fence) {
      const body: string[] = [];
      index++;
      while (index < lines.length && !lines[index].trim().startsWith(fence[1])) body.push(lines[index++]);
      if (index < lines.length) index++;
      blocks.push({ kind: "code", language: fence[2], text: body.join("\n") });
      continue;
    }
    const heading = /^(#{1,6})\s+(.+)$/.exec(line);
    if (heading) { blocks.push({ kind: "heading", level: heading[1].length, text: heading[2] }); index++; continue; }
    if (/^\s*(---+|\*\*\*+)\s*$/.test(line)) { blocks.push({ kind: "rule" }); index++; continue; }
    if (/^>\s?/.test(line)) {
      const quote: string[] = [];
      while (index < lines.length && /^>\s?/.test(lines[index])) quote.push(lines[index++].replace(/^>\s?/, ""));
      blocks.push({ kind: "quote", text: quote.join("\n") }); continue;
    }
    const list = /^\s*(?:([-*+])|(\d+)\.)\s+(.+)$/.exec(line);
    if (list) {
      const ordered = Boolean(list[2]); const items: string[] = [];
      while (index < lines.length) {
        const item = /^\s*(?:([-*+])|(\d+)\.)\s+(.+)$/.exec(lines[index]);
        if (!item || Boolean(item[2]) !== ordered) break;
        items.push(item[3]); index++;
      }
      blocks.push({ kind: "list", ordered, items }); continue;
    }
    const paragraph = [line]; index++;
    while (index < lines.length && lines[index].trim() && !/^(?:#{1,6}\s|>\s?|\s*[-*+]\s|\s*\d+\.\s|\s*`{3}|\s*~{3})/.test(lines[index])) paragraph.push(lines[index++]);
    blocks.push({ kind: "paragraph", text: paragraph.join("\n") });
  }
  return blocks;
}
