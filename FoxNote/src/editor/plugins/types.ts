import type { NoteBlock } from "../../types/note";

export interface BlockPlugin {
  type: string;
  label: string;
  description: string;
  createDefaultBlock: () => NoteBlock;
}
