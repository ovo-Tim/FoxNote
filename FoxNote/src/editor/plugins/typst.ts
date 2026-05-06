import type { NoteBlock } from "../../types/note";
import type { BlockPlugin } from "./types";

const defaultTypstContent = "";

export const typstBlockPlugin: BlockPlugin = {
  type: "typst",
  label: "Typst",
  description: "Code and live preview",
  createDefaultBlock: (): NoteBlock => ({
    type: "typst",
    content: defaultTypstContent,
  }),
};
