import type { NoteBlock } from "../types/note";

function safeInlineContent(block: NoteBlock): string {
  if (typeof block.content === "string") {
    return block.content;
  }
  return "";
}

export function blockPreviewText(block: NoteBlock): string {
  if (block.type === "typst") {
    const content = safeInlineContent(block);
    if (content.trim().length === 0) {
      return "Click to start writing typst content...";
    }
    return content;
  }

  if (block.path) {
    return `[${block.type}] external content at ${block.path}`;
  }
  if (safeInlineContent(block).trim().length > 0) {
    return safeInlineContent(block);
  }
  return `[${block.type}]`;
}

export function cloneBlocks(blocks: NoteBlock[]): NoteBlock[] {
  return blocks.map((block) => ({ ...block }));
}
