import TitleBlockCard from "../../components/blocks/TitleBlockCard.vue";
import type { NoteBlock } from "../../types/note";
import type { BlockPlugin, EditorPlugin } from "./types";

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
