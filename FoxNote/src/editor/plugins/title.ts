import TitleBlockCard from "../../components/blocks/TitleBlockCard.vue";
import { exportTextAsGraphic } from "../../lib/blockExport";
import type { NoteBlock } from "../../types/note";
import type { BlockPlugin, EditorPlugin } from "./types";

function escapeTypstText(input: string): string {
  return input.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
}

function typstHeadingLine(level: number, text: string): string {
  const normalizedLevel = Math.min(6, Math.max(1, level));
  return `${"=".repeat(normalizedLevel)} ${escapeTypstText(text || "Untitled section")}`;
}

const titleBlockPlugin: BlockPlugin = {
  type: "title",
  label: "Title",
  description: "Foldable section title",
  createDefaultBlock: (): NoteBlock => ({
    type: "title",
    level: 1,
    content: "Untitled section",
    folded: false,
    summary: "",
  }),
  previewText: (block) => {
    const level = Math.min(6, Math.max(1, Number(block.level) || 1));
    const text = typeof block.content === "string" ? block.content.trim() : "";
    if (text) {
      return `${"#".repeat(level)} ${text}`;
    }
    return `${"#".repeat(level)} Untitled section`;
  },
  renderer: {
    component: TitleBlockCard,
    props: (context) => ({
      modelValue: typeof context.block.content === "string" ? context.block.content : "",
      level: Math.min(6, Math.max(1, Number(context.block.level) || 1)),
      folded: Boolean(context.block.folded),
      summary: typeof context.block.summary === "string" ? context.block.summary : "",
      editing: context.editing,
    }),
    on: (context) => ({
      focus: context.setEditing,
      blur: context.clearEditing,
      updateModelValue: (value: string) => context.updateContent(String(value ?? "")),
      updateLevel: (value: number) => context.updateLevel(value),
      updateFolded: (value: boolean) => context.updateFolded(Boolean(value)),
      updateSummary: (value: string) => context.updateSummary(String(value ?? "")),
    }),
  },
  exportBlock: async (context, format) => {
    const level = Math.min(6, Math.max(1, Number(context.block.level) || 1));
    const text = typeof context.block.content === "string" ? context.block.content.trim() : "";
    const width = Math.max(320, Math.min(2200, Math.round(Number(context.block.width) || 960)));
    const fontSize = Math.max(16, 42 - level * 4);
    const height = Math.max(70, Math.round(fontSize * 2.2));
    const result = await exportTextAsGraphic(text || "Untitled section", format, {
      width,
      height,
      fontSize,
      fontWeight: 700 - level * 40,
      color: "#121826",
      background: "transparent",
    });
    return {
      format,
      mimeType: result.mimeType,
      bytes: result.bytes,
      width: result.width,
      height: result.height,
    };
  },
  exportTypst: async (context) => {
    const level = Math.min(6, Math.max(1, Number(context.block.level) || 1));
    const text = typeof context.block.content === "string" ? context.block.content.trim() : "";
    return {
      typst: typstHeadingLine(level, text),
    };
  },
};

const titleOneBlockPlugin: BlockPlugin = {
  type: "title1",
  label: "Title 1",
  description: "Insert H1 title block",
  createDefaultBlock: (): NoteBlock => ({
    type: "title",
    level: 1,
    content: "Untitled H1",
    folded: false,
    summary: "",
  }),
  previewText: () => "# Untitled H1",
};

const titleTwoBlockPlugin: BlockPlugin = {
  type: "title2",
  label: "Title 2",
  description: "Insert H2 title block",
  createDefaultBlock: (): NoteBlock => ({
    type: "title",
    level: 2,
    content: "Untitled H2",
    folded: false,
    summary: "",
  }),
  previewText: () => "## Untitled H2",
};

const titleThreeBlockPlugin: BlockPlugin = {
  type: "title3",
  label: "Title 3",
  description: "Insert H3 title block",
  createDefaultBlock: (): NoteBlock => ({
    type: "title",
    level: 3,
    content: "Untitled H3",
    folded: false,
    summary: "",
  }),
  previewText: () => "### Untitled H3",
};

export const titleEditorPlugin: EditorPlugin = {
  id: "builtin.title",
  name: "Title",
  blocks: [
    titleBlockPlugin,
    titleOneBlockPlugin,
    titleTwoBlockPlugin,
    titleThreeBlockPlugin,
  ],
};

export {
  titleBlockPlugin,
  titleOneBlockPlugin,
  titleTwoBlockPlugin,
  titleThreeBlockPlugin,
};
export default titleEditorPlugin;
