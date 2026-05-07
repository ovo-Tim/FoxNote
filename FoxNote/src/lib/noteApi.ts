import { invoke } from "@tauri-apps/api/core";
import type {
  ExportTypstResult,
  FolderEntry,
  InstallPluginInput,
  NoteAttachmentPayload,
  NoteDocument,
  NoteRecord,
  NoteSearchHit,
  PluginEntry,
  SyncStatus,
  TagEntry,
  NoteTree,
} from "../types/note";

export async function getNoteStorageRoot(): Promise<string> {
  return invoke<string>("get_note_storage_root");
}

export async function listNoteTree(): Promise<NoteTree> {
  return invoke<NoteTree>("list_note_tree");
}

export async function createNoteFolder(
  parent: string,
  name: string,
): Promise<FolderEntry> {
  return invoke<FolderEntry>("create_note_folder", { parent, name });
}

export async function renameNoteFolder(
  path: string,
  name: string,
): Promise<FolderEntry> {
  return invoke<FolderEntry>("rename_note_folder", { path, name });
}

export async function deleteNoteFolder(path: string): Promise<void> {
  return invoke<void>("delete_note_folder", { path });
}

export async function createNote(
  folder: string,
  title: string,
): Promise<NoteRecord> {
  return invoke<NoteRecord>("create_note", { folder, title });
}

export async function loadNote(id: string): Promise<NoteRecord> {
  return invoke<NoteRecord>("load_note", { id });
}

export async function saveNote(
  id: string,
  document: NoteDocument,
): Promise<NoteRecord> {
  return invoke<NoteRecord>("save_note", { id, document });
}

export async function deleteNote(id: string): Promise<void> {
  return invoke<void>("delete_note", { id });
}

export async function exportNoteTypst(
  id: string,
  outputDir?: string,
): Promise<ExportTypstResult> {
  return invoke<ExportTypstResult>("export_note_typst", { id, outputDir });
}

export async function saveNoteImageAttachment(
  id: string,
  mimeType: string,
  bytes: number[],
): Promise<string> {
  return invoke<string>("save_note_image_attachment", { id, mimeType, bytes });
}

export async function loadNoteImageAttachment(
  id: string,
  path: string,
): Promise<NoteAttachmentPayload> {
  return invoke<NoteAttachmentPayload>("load_note_image_attachment", { id, path });
}

export async function getTagBridgePath(): Promise<string> {
  return invoke<string>("get_tag_bridge_path");
}

export async function getSyncConfigPath(): Promise<string> {
  return invoke<string>("get_sync_config_path");
}

export async function getPluginConfigPath(): Promise<string> {
  return invoke<string>("get_plugin_config_path");
}

export async function listTags(): Promise<TagEntry[]> {
  return invoke<TagEntry[]>("list_tags");
}

export async function searchNotes(
  query: string,
  tagPath?: string,
  limit = 80,
): Promise<NoteSearchHit[]> {
  return invoke<NoteSearchHit[]>("search_notes", { query, tagPath, limit });
}

export async function listNoteIdsForTag(tag: string): Promise<string[]> {
  return invoke<string[]>("list_note_ids_for_tag", { tag });
}

export async function importTagBridge(): Promise<TagEntry[]> {
  return invoke<TagEntry[]>("import_tag_bridge");
}

export async function rebuildTagIndex(): Promise<TagEntry[]> {
  return invoke<TagEntry[]>("rebuild_tag_index");
}

export async function getSyncStatus(): Promise<SyncStatus> {
  return invoke<SyncStatus>("get_sync_status");
}

export async function initSyncRepository(): Promise<SyncStatus> {
  return invoke<SyncStatus>("init_sync_repository");
}

export async function setSyncRemoteUrl(remoteUrl: string): Promise<SyncStatus> {
  return invoke<SyncStatus>("set_sync_remote_url", { remoteUrl });
}

export async function runSyncNow(): Promise<SyncStatus> {
  return invoke<SyncStatus>("run_sync_now");
}

export async function runPullOnly(): Promise<SyncStatus> {
  return invoke<SyncStatus>("run_pull_only");
}

export async function runPullThenPush(): Promise<SyncStatus> {
  return invoke<SyncStatus>("run_pull_then_push");
}

export async function runCommitOnly(message: string): Promise<SyncStatus> {
  return invoke<SyncStatus>("run_commit_only", { message });
}

export async function runCommitNoteOnly(noteId: string, message: string): Promise<SyncStatus> {
  return invoke<SyncStatus>("run_commit_note_only", { noteId, message });
}

export async function noteHasChanges(noteId: string): Promise<boolean> {
  return invoke<boolean>("note_has_changes", { noteId });
}

export async function setAutoSync(enabled: boolean, intervalSec: number): Promise<SyncStatus> {
  return invoke<SyncStatus>("set_auto_sync", { enabled, intervalSec });
}

export async function resolveSyncConflict(noteId: string, useLocal: boolean): Promise<SyncStatus> {
  return invoke<SyncStatus>("resolve_sync_conflict", { noteId, useLocal });
}

export async function finalizeSyncConflicts(): Promise<SyncStatus> {
  return invoke<SyncStatus>("finalize_sync_conflicts");
}

export async function listPlugins(): Promise<PluginEntry[]> {
  return invoke<PluginEntry[]>("list_plugins");
}

export async function installPlugin(input: InstallPluginInput): Promise<PluginEntry> {
  return invoke<PluginEntry>("install_plugin", { input });
}

export async function setPluginEnabled(pluginId: string, enabled: boolean): Promise<PluginEntry> {
  return invoke<PluginEntry>("set_plugin_enabled", { pluginId, enabled });
}

export async function removePlugin(pluginId: string): Promise<void> {
  return invoke<void>("remove_plugin", { pluginId });
}
