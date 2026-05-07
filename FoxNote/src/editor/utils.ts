import type { NoteBlock } from "../types/note";
import { getBlockPlugin } from "./plugins/registry";

function safeInlineContent(block: NoteBlock): string {
  if (typeof block.content === "string") {
    return block.content;
  }
  return "";
}

export function blockPreviewText(block: NoteBlock): string {
  const plugin = getBlockPlugin(block.type);
  if (plugin?.previewText) {
    return plugin.previewText(block);
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
