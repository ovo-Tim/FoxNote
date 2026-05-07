import CanvasBlockCard from "../../components/blocks/CanvasBlockCard.vue";
import type { NoteBlock } from "../../types/note";
import type { BlockPlugin, EditorPlugin } from "./types";

const canvasBlockPlugin: BlockPlugin = {
  type: "canvas",
  label: "Canvas",
  description: "tldraw whiteboard canvas",
  createDefaultBlock: (): NoteBlock => ({
    type: "canvas",
    path: "",
  }),
  previewText: (block) => {
    const path = block.path?.trim() ?? "";
    return path ? `[canvas] ${path}` : "[canvas] unsaved";
  },
  renderer: {
    component: CanvasBlockCard,
    props: (context) => ({
      noteId: context.note?.id ?? "",
      path: String(context.block.path ?? ""),
      editing: context.editing,
      width: Number(context.block.width ?? 0),
      height: Number(context.block.height ?? 0),
    }),
    on: (context) => ({
      focus: context.setEditing,
      updatePath: (path: string) => context.updatePath(String(path ?? "")),
      updateSize: (width: number, height: number) => context.updateSize(width, height),
    }),
  },
};

export const canvasEditorPlugin: EditorPlugin = {
  id: "builtin.canvas",
  name: "Canvas",
  blocks: [canvasBlockPlugin],
};

export { canvasBlockPlugin };
export default canvasEditorPlugin;
