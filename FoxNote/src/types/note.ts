export interface NoteBlock {
  type: string;
  content?: string | null;
  path?: string | null;
}

export interface NoteDocument {
  title: string;
  date: string;
  type: string;
  tags: string[];
  content: NoteBlock[];
}

export interface NoteRecord {
  id: string;
  folder: string;
  document: NoteDocument;
}

export interface NoteSummary {
  id: string;
  folder: string;
  title: string;
  date: string;
  type: string;
  tags: string[];
}

export interface FolderEntry {
  path: string;
  name: string;
}

export interface NoteTree {
  folders: FolderEntry[];
  notes: NoteSummary[];
}

export interface TagEntry {
  path: string;
  name: string;
  noteCount: number;
}
