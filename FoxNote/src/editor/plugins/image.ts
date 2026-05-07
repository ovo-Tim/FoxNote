import type { NoteBlock } from "../../types/note";
import type { BlockPlugin } from "./types";

export const imageBlockPlugin: BlockPlugin = {
  type: "image",
  label: "Image",
  description: "Image attachment preview",
  createDefaultBlock: (): NoteBlock => ({
    type: "image",
    path: "",
  }),
};
