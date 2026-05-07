export interface NoteBlock {
  type: string;
  content?: string | null;
  path?: string | null;
  width?: number | null;
  height?: number | null;
  level?: number | null;
  folded?: boolean | null;
  summary?: string | null;
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

export interface ExportTypstResult {
  exportDir: string;
  exportFile: string;
  attachmentCount: number;
}

export interface NoteAttachmentPayload {
  mimeType: string;
  bytes: number[];
}

export interface NoteSummary {
  id: string;
  folder: string;
  title: string;
  date: string;
  type: string;
  tags: string[];
}

export interface NoteSearchHit {
  id: string;
  folder: string;
  title: string;
  date: string;
  type: string;
  tags: string[];
  snippet: string;
  score: number;
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

export type SyncPhase = "needs_setup" | "idle" | "syncing" | "conflict" | "error";

export interface SyncConflict {
  noteId: string;
  files: string[];
}

export interface SyncStatus {
  phase: SyncPhase;
  repoInitialized: boolean;
  remoteUrl?: string | null;
  branch?: string | null;
  autoSyncEnabled: boolean;
  autoSyncIntervalSec: number;
  lastSyncAt?: string | null;
  message?: string | null;
  conflicts: SyncConflict[];
}

export type PluginSourceKind = "local" | "remote";

export interface PluginEntry {
  id: string;
  name: string;
  sourceKind: PluginSourceKind;
  source: string;
  enabled: boolean;
  installedAt: string;
}

export interface InstallPluginInput {
  id: string;
  name: string;
  sourceKind: PluginSourceKind;
  source: string;
}
