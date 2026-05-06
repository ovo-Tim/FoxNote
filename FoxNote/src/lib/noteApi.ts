import { invoke } from "@tauri-apps/api/core";
import type {
  FolderEntry,
  NoteDocument,
  NoteRecord,
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

export async function getTagBridgePath(): Promise<string> {
  return invoke<string>("get_tag_bridge_path");
}

export async function listTags(): Promise<TagEntry[]> {
  return invoke<TagEntry[]>("list_tags");
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
