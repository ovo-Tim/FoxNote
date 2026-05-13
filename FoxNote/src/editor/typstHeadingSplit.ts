import type { NoteBlock } from "../types/note";

export function splitTypstContentIntoBlocks(content: string): NoteBlock[] | null {
  const normalized = content.replace(/\r\n/g, "\n");
  const lines = normalized.split("\n");
  const blocks: NoteBlock[] = [];
  const typstLines: string[] = [];
  let foundHeading = false;
  let inCodeFence = false;

  const flushTypstLines = () => {
    const chunk = typstLines.join("\n").trim();
    typstLines.length = 0;
    if (!chunk) {
      return;
    }

    blocks.push({
      type: "typst",
      content: chunk,
    });
  };

  for (let cursor = 0; cursor < lines.length; cursor += 1) {
    const line = lines[cursor] ?? "";
    const trimmed = line.trim();

    if (trimmed.startsWith("```")) {
      inCodeFence = !inCodeFence;
      typstLines.push(line);
      continue;
    }

    if (inCodeFence) {
      typstLines.push(line);
      continue;
    }

    const match = trimmed.match(/^(={1,6})\s+(.+)$/);
    if (match) {
      const title = (match[2] ?? "").trim();
      if (!title) {
        typstLines.push(line);
        continue;
      }

      flushTypstLines();
      blocks.push({
        type: "title",
        level: (match[1] ?? "=").length,
        content: title,
        folded: false,
        summary: "",
      });
      foundHeading = true;
      continue;
    }

    typstLines.push(line);
  }

  flushTypstLines();

  if (!foundHeading || blocks.length === 0) {
    return null;
  }

  return blocks;
}
