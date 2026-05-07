import TypstBlockCard from "../../components/blocks/TypstBlockCard.vue";
import type { NoteBlock } from "../../types/note";
import type { BlockPlugin, EditorPlugin } from "./types";

const defaultTypstContent = "";

const typstBlockPlugin: BlockPlugin = {
  type: "typst",
  label: "Typst",
  description: "Code and live preview",
  createDefaultBlock: (): NoteBlock => ({
    type: "typst",
    content: defaultTypstContent,
  }),
  previewText: (block) => {
    const content = typeof block.content === "string" ? block.content : "";
    if (!content.trim()) {
      return "Click to start writing typst content...";
    }
    return content;
  },
  renderer: {
    component: TypstBlockCard,
    props: (context) => ({
      modelValue: typeof context.block.content === "string" ? context.block.content : "",
      editing: context.editing,
    }),
    on: (context) => ({
      focus: context.setEditing,
      blur: context.clearEditing,
      updateModelValue: (value: string) => context.updateContent(String(value ?? "")),
    }),
  },
};

export const typstEditorPlugin: EditorPlugin = {
  id: "builtin.typst",
  name: "Typst",
  blocks: [typstBlockPlugin],
};

export { typstBlockPlugin };
export default typstEditorPlugin;
