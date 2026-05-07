import ImageBlockCard from "../../components/blocks/ImageBlockCard.vue";
import type { NoteBlock } from "../../types/note";
import type { BlockPlugin, EditorPlugin } from "./types";

const imageBlockPlugin: BlockPlugin = {
  type: "image",
  label: "Image",
  description: "Image attachment preview",
  createDefaultBlock: (): NoteBlock => ({
    type: "image",
    path: "",
  }),
  previewText: (block) => {
    if (block.path?.trim()) {
      return `[image] ${block.path}`;
    }
    return "[image] attachment missing";
  },
  renderer: {
    component: ImageBlockCard,
    props: (context) => ({
      noteId: context.note?.id ?? "",
      path: String(context.block.path ?? ""),
      editing: context.editing,
    }),
    on: (context) => ({
      focus: context.setEditing,
      updatePath: (path: string) => context.updatePath(String(path ?? "")),
    }),
  },
};

const imageCaptionBlockPlugin: BlockPlugin = {
  type: "image_caption",
  label: "Image caption",
  description: "Caption text for nearby images",
  createDefaultBlock: (): NoteBlock => ({
    type: "image_caption",
    content: "Caption",
  }),
  previewText: (block) => {
    const content = typeof block.content === "string" ? block.content.trim() : "";
    return content ? `[caption] ${content}` : "[caption]";
  },
};

export const imageEditorPlugin: EditorPlugin = {
  id: "builtin.image",
  name: "Image",
  blocks: [imageBlockPlugin, imageCaptionBlockPlugin],
};

export { imageBlockPlugin, imageCaptionBlockPlugin };
export default imageEditorPlugin;
