mod core;

use core::{
    notes::{FolderEntry, NoteDocument, NoteRecord, NoteService, NoteTree, SaveNoteInput},
    tags::{TagEntry, TagIndexService},
};
use std::path::Path;
use tauri::Manager;

struct AppState {
    note_service: NoteService,
    tag_service: TagIndexService,
    note_root: String,
    tag_bridge_path: String,
}

impl AppState {
    fn from_app(app: &tauri::App) -> Result<Self, String> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data directory: {error}"))?;

        let note_root = app_data_dir.join("notes");
        let note_service = NoteService::new(&note_root).map_err(|error| error.to_string())?;

        let tag_db_path = app_data_dir.join("tag-index.sqlite3");
        let tag_bridge_path = note_root.join("tags-index.toml");
        let tag_service = TagIndexService::new(tag_db_path, &tag_bridge_path)
            .map_err(|error| error.to_string())?;

        sync_tag_index_from_notes(&note_service, &tag_service)?;

        Ok(Self {
            note_service,
            tag_service,
            note_root: path_to_string(&note_root),
            tag_bridge_path: path_to_string(&tag_bridge_path),
        })
    }
}

fn sync_tag_index_from_notes(
    note_service: &NoteService,
    tag_service: &TagIndexService,
) -> Result<(), String> {
    let tree = note_service
        .list_tree()
        .map_err(|error| format!("failed to list notes for tag indexing: {error}"))?;

    let note_entries = tree
        .notes
        .into_iter()
        .map(|note| (note.id, note.tags))
        .collect::<Vec<(String, Vec<String>)>>();

    tag_service
        .replace_all(&note_entries)
        .map_err(|error| format!("failed to rebuild tag index from notes: {error}"))
}

#[tauri::command]
fn get_note_storage_root(state: tauri::State<AppState>) -> String {
    state.note_root.clone()
}

#[tauri::command]
fn get_tag_bridge_path(state: tauri::State<AppState>) -> String {
    state.tag_bridge_path.clone()
}

#[tauri::command]
fn list_note_tree(state: tauri::State<AppState>) -> Result<NoteTree, String> {
    state
        .note_service
        .list_tree()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_note_folder(
    state: tauri::State<AppState>,
    parent: String,
    name: String,
) -> Result<FolderEntry, String> {
    state
        .note_service
        .create_folder(&parent, &name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_note_folder(
    state: tauri::State<AppState>,
    path: String,
    name: String,
) -> Result<FolderEntry, String> {
    let renamed = state
        .note_service
        .rename_folder(&path, &name)
        .map_err(|error| error.to_string())?;

    sync_tag_index_from_notes(&state.note_service, &state.tag_service)?;

    Ok(renamed)
}

#[tauri::command]
fn delete_note_folder(state: tauri::State<AppState>, path: String) -> Result<(), String> {
    state
        .note_service
        .delete_folder(&path)
        .map_err(|error| error.to_string())?;

    sync_tag_index_from_notes(&state.note_service, &state.tag_service)
}

#[tauri::command]
fn create_note(
    state: tauri::State<AppState>,
    folder: String,
    title: String,
) -> Result<NoteRecord, String> {
    let record = state
        .note_service
        .create_note(&folder, &title)
        .map_err(|error| error.to_string())?;

    state
        .tag_service
        .upsert_note_tags(&record.id, &record.document.tags)
        .map_err(|error| error.to_string())?;

    Ok(record)
}

#[tauri::command]
fn load_note(state: tauri::State<AppState>, id: String) -> Result<NoteRecord, String> {
    state
        .note_service
        .load_note(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_note(
    state: tauri::State<AppState>,
    id: String,
    document: NoteDocument,
) -> Result<NoteRecord, String> {
    let record = state
        .note_service
        .save_note(SaveNoteInput { id, document })
        .map_err(|error| error.to_string())?;

    state
        .tag_service
        .upsert_note_tags(&record.id, &record.document.tags)
        .map_err(|error| error.to_string())?;

    Ok(record)
}

#[tauri::command]
fn delete_note(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    state
        .note_service
        .delete_note(&id)
        .map_err(|error| error.to_string())?;

    state
        .tag_service
        .remove_note(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_tags(state: tauri::State<AppState>) -> Result<Vec<TagEntry>, String> {
    state
        .tag_service
        .list_tags()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_note_ids_for_tag(
    state: tauri::State<AppState>,
    tag: String,
) -> Result<Vec<String>, String> {
    state
        .tag_service
        .note_ids_for_tag(&tag)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn import_tag_bridge(state: tauri::State<AppState>) -> Result<Vec<TagEntry>, String> {
    state
        .tag_service
        .import_from_export()
        .map_err(|error| error.to_string())?;

    state
        .tag_service
        .list_tags()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rebuild_tag_index(state: tauri::State<AppState>) -> Result<Vec<TagEntry>, String> {
    sync_tag_index_from_notes(&state.note_service, &state.tag_service)?;
    state
        .tag_service
        .list_tags()
        .map_err(|error| error.to_string())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = AppState::from_app(app).map_err(std::io::Error::other)?;
            app.manage(state);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_note_storage_root,
            list_note_tree,
            create_note_folder,
            rename_note_folder,
            delete_note_folder,
            create_note,
            load_note,
            save_note,
            delete_note,
            get_tag_bridge_path,
            list_tags,
            list_note_ids_for_tag,
            import_tag_bridge,
            rebuild_tag_index
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
