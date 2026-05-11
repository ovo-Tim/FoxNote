mod core;

use core::{
    notes::{
        ExportTypstResult, FolderEntry, NoteAttachmentPayload, NoteDocument, NoteRecord,
        NoteService, NoteTree, SaveNoteInput,
    },
    plugins::{InstallPluginInput, PluginEntry, PluginService},
    search::{NoteSearchHit, SearchService},
    sync::{SyncService, SyncStatus},
    tags::{TagEntry, TagIndexService},
};
use std::{fs, path::Path};
use tauri::Manager;
use typst::layout::PagedDocument;
use typst_as_lib::typst_kit_options::TypstKitFontOptions;
use typst_as_lib::TypstEngine;

static SOURCE_HAN_SERIF_SC_REGULAR: &[u8] = include_bytes!("../fonts/SourceHanSerifSC-Regular.otf");
static SOURCE_HAN_SERIF_SC_BOLD: &[u8] = include_bytes!("../fonts/SourceHanSerifSC-Bold.otf");

struct AppState {
    note_service: NoteService,
    tag_service: TagIndexService,
    sync_service: SyncService,
    plugin_service: PluginService,
    search_service: SearchService,
    note_root: String,
    tag_bridge_path: String,
    sync_config_path: String,
    plugin_config_path: String,
}

impl AppState {
    fn from_app(app: &tauri::App) -> Result<Self, String> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data directory: {error}"))?;

        let note_root = app_data_dir.join("notes");
        let note_service = NoteService::new(&note_root).map_err(|error| error.to_string())?;
        let sync_service = SyncService::new(&note_root);
        let sync_config_path = sync_service.config_path_string();
        let plugin_service = PluginService::new(&note_root).map_err(|error| error.to_string())?;
        let plugin_config_path = plugin_service.config_path_string();
        let search_index_path = app_data_dir.join("tantivy-search");
        let search_service =
            SearchService::new(search_index_path).map_err(|error| error.to_string())?;

        let tag_db_path = app_data_dir.join("tag-index.sqlite3");
        let tag_bridge_path = note_root.join("tags-index.toml");
        let tag_service = TagIndexService::new(tag_db_path, &tag_bridge_path)
            .map_err(|error| error.to_string())?;

        sync_indexes_from_notes(&note_service, &tag_service, &search_service)?;

        Ok(Self {
            note_service,
            tag_service,
            sync_service,
            plugin_service,
            search_service,
            note_root: path_to_string(&note_root),
            tag_bridge_path: path_to_string(&tag_bridge_path),
            sync_config_path,
            plugin_config_path,
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

fn sync_search_index_from_notes(
    note_service: &NoteService,
    search_service: &SearchService,
) -> Result<(), String> {
    search_service
        .rebuild_from_notes(note_service)
        .map_err(|error| format!("failed to rebuild search index from notes: {error}"))
}

fn sync_indexes_from_notes(
    note_service: &NoteService,
    tag_service: &TagIndexService,
    search_service: &SearchService,
) -> Result<(), String> {
    sync_tag_index_from_notes(note_service, tag_service)?;
    sync_search_index_from_notes(note_service, search_service)
}

fn typst_raw_literal(input: &str) -> String {
    let mut fence_len = 1usize;
    while input.contains(&"`".repeat(fence_len)) {
        fence_len += 1;
    }

    let fence = "`".repeat(fence_len);
    format!("{fence}{input}{fence}")
}

fn inline_latex_to_typst(latex: &str) -> String {
    let trimmed = latex.trim();
    if trimmed.is_empty() {
        return "$ $".to_string();
    }

    format!("#mi({})", typst_raw_literal(trimmed))
}

fn block_latex_to_typst(latex: &str) -> String {
    let trimmed = latex.trim();
    if trimmed.is_empty() {
        return "".to_string();
    }

    format!("#mitex({})", typst_raw_literal(trimmed))
}

fn convert_inline_latex_math(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::new();
    let mut index = 0;

    while index < chars.len() {
        let current = chars[index];
        if current == '$' && (index == 0 || chars[index - 1] != '\\') {
            let mut end = index + 1;
            while end < chars.len() {
                if chars[end] == '$' && chars[end - 1] != '\\' {
                    break;
                }
                end += 1;
            }

            if end < chars.len() {
                let latex: String = chars[index + 1..end].iter().collect();
                result.push_str(&inline_latex_to_typst(&latex));
                index = end + 1;
                continue;
            }
        }

        result.push(current);
        index += 1;
    }

    result
}

fn convert_markdown_to_typst(markdown: &str) -> String {
    let normalized = markdown.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut output: Vec<String> = Vec::new();
    let mut index = 0;
    let mut in_code_fence = false;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            output.push(trimmed.to_string());
            index += 1;
            continue;
        }

        if in_code_fence {
            output.push(line.to_string());
            index += 1;
            continue;
        }

        if trimmed.starts_with("$$") {
            let mut block_math = String::new();
            let mut closed = false;

            if trimmed != "$$" {
                block_math.push_str(trimmed.trim_start_matches("$$"));
                if block_math.ends_with("$$") {
                    block_math.truncate(block_math.len().saturating_sub(2));
                    closed = true;
                }
            }

            index += 1;
            while !closed && index < lines.len() {
                let math_line = lines[index];
                if math_line.trim_end().ends_with("$$") {
                    let content = math_line.trim_end_matches('$');
                    if !block_math.is_empty() && !content.is_empty() {
                        block_math.push('\n');
                    }
                    block_math.push_str(content);
                    index += 1;
                    break;
                }

                if !block_math.is_empty() {
                    block_math.push('\n');
                }
                block_math.push_str(math_line);
                index += 1;
            }

            let converted = block_latex_to_typst(block_math.trim());
            if !converted.is_empty() {
                output.push(converted);
            }
            continue;
        }

        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|ch| *ch == '#').count();
            let title = trimmed[level..].trim();
            if (1..=6).contains(&level) && !title.is_empty() {
                output.push(format!(
                    "{} {}",
                    "=".repeat(level),
                    convert_inline_latex_math(title)
                ));
                index += 1;
                continue;
            }
        }

        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            output.push(format!("- {}", convert_inline_latex_math(rest.trim())));
            index += 1;
            continue;
        }

        let ordered_start = trimmed
            .char_indices()
            .find(|(_, ch)| !ch.is_ascii_digit())
            .map(|(idx, ch)| (idx, ch));
        if let Some((marker_index, marker)) = ordered_start {
            if (marker == '.' || marker == ')') && trimmed[marker_index + 1..].starts_with(' ') {
                let item = trimmed[marker_index + 2..].trim();
                output.push(format!("+ {}", convert_inline_latex_math(item)));
                index += 1;
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix(">") {
            output.push(format!(
                "#quote[{}]",
                convert_inline_latex_math(rest.trim())
            ));
            index += 1;
            continue;
        }

        output.push(convert_inline_latex_math(line));
        index += 1;
    }

    output.join("\n").trim().to_string()
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
fn get_sync_config_path(state: tauri::State<AppState>) -> String {
    state.sync_config_path.clone()
}

#[tauri::command]
fn get_plugin_config_path(state: tauri::State<AppState>) -> String {
    state.plugin_config_path.clone()
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

    sync_indexes_from_notes(
        &state.note_service,
        &state.tag_service,
        &state.search_service,
    )?;

    Ok(renamed)
}

#[tauri::command]
fn delete_note_folder(state: tauri::State<AppState>, path: String) -> Result<(), String> {
    state
        .note_service
        .delete_folder(&path)
        .map_err(|error| error.to_string())?;

    sync_indexes_from_notes(
        &state.note_service,
        &state.tag_service,
        &state.search_service,
    )
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

    state
        .search_service
        .upsert_note_record(&record)
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

    state
        .search_service
        .upsert_note_record(&record)
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
        .map_err(|error| error.to_string())?;

    state
        .search_service
        .remove_note(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn search_notes(
    state: tauri::State<AppState>,
    query: String,
    tag_path: Option<String>,
    include_tags: Option<Vec<String>>,
    exclude_tags: Option<Vec<String>>,
    match_all_includes: Option<bool>,
    limit: Option<usize>,
) -> Result<Vec<NoteSearchHit>, String> {
    let include_tags = include_tags.unwrap_or_default();
    let exclude_tags = exclude_tags.unwrap_or_default();
    let match_all_includes = match_all_includes.unwrap_or(true);

    state
        .search_service
        .search(
            &query,
            tag_path.as_deref(),
            &include_tags,
            &exclude_tags,
            match_all_includes,
            limit.unwrap_or(80),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn export_note_typst(
    state: tauri::State<AppState>,
    id: String,
    output_dir: Option<String>,
) -> Result<ExportTypstResult, String> {
    state
        .note_service
        .export_note_typst(&id, output_dir.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn write_export_file(path: String, bytes: Vec<u8>) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("output path cannot be empty".to_string());
    }

    let output = Path::new(trimmed);
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
    }

    fs::write(output, bytes).map_err(|error| error.to_string())
}

#[tauri::command]
fn compile_typst_to_pdf(input_path: String, output_path: String) -> Result<(), String> {
    let input = input_path.trim();
    let output = output_path.trim();

    if input.is_empty() {
        return Err("typst input path cannot be empty".to_string());
    }
    if output.is_empty() {
        return Err("pdf output path cannot be empty".to_string());
    }

    let input_file = Path::new(input);
    let output_file = Path::new(output);

    if let Some(parent) = output_file.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
    }

    let package_dir = input_file
        .parent()
        .ok_or_else(|| "cannot resolve export package directory".to_string())?;

    let main_source = fs::read_to_string(input_file)
        .map_err(|error| format!("failed to read typst input: {error}"))?;

    let bundled_font_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts");

    let engine = TypstEngine::builder()
        .main_file(("main.typ", main_source))
        .fonts([SOURCE_HAN_SERIF_SC_REGULAR, SOURCE_HAN_SERIF_SC_BOLD])
        .search_fonts_with(
            TypstKitFontOptions::default()
                .include_system_fonts(false)
                .include_dirs([bundled_font_dir]),
        )
        .with_file_system_resolver(package_dir.to_path_buf())
        .with_package_file_resolver()
        .build();

    let compiled = engine
        .compile::<PagedDocument>()
        .output
        .map_err(|error| format!("embedded typst compile failed: {error}"))?;

    let pdf_bytes = typst_pdf::pdf(&compiled, &Default::default())
        .map_err(|error| format!("failed to generate pdf bytes: {error:?}"))?;

    fs::write(output_file, pdf_bytes).map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
fn convert_markdown_text_to_typst(markdown: String) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("markdown input cannot be empty".to_string());
    }

    Ok(convert_markdown_to_typst(&markdown))
}

#[tauri::command]
fn save_note_image_attachment(
    state: tauri::State<AppState>,
    id: String,
    mime_type: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    state
        .note_service
        .save_note_image_attachment(&id, &mime_type, &bytes)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn load_note_image_attachment(
    state: tauri::State<AppState>,
    id: String,
    path: String,
) -> Result<NoteAttachmentPayload, String> {
    state
        .note_service
        .load_note_image_attachment(&id, &path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_note_attachment(
    state: tauri::State<AppState>,
    id: String,
    mime_type: String,
    bytes: Vec<u8>,
    base_name: Option<String>,
    extension: Option<String>,
) -> Result<String, String> {
    state
        .note_service
        .save_note_attachment(
            &id,
            &mime_type,
            &bytes,
            base_name.as_deref(),
            extension.as_deref(),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn load_note_attachment(
    state: tauri::State<AppState>,
    id: String,
    path: String,
    mime_type: Option<String>,
) -> Result<NoteAttachmentPayload, String> {
    state
        .note_service
        .load_note_attachment(&id, &path, mime_type.as_deref())
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

#[tauri::command]
fn get_sync_status(state: tauri::State<AppState>) -> Result<SyncStatus, String> {
    state
        .sync_service
        .status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn init_sync_repository(state: tauri::State<AppState>) -> Result<SyncStatus, String> {
    state
        .sync_service
        .init_repo()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_sync_remote_url(
    state: tauri::State<AppState>,
    remote_url: String,
) -> Result<SyncStatus, String> {
    state
        .sync_service
        .set_remote(&remote_url)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn run_sync_now(state: tauri::State<AppState>) -> Result<SyncStatus, String> {
    let status = state
        .sync_service
        .sync_now()
        .map_err(|error| error.to_string())?;

    sync_indexes_from_notes(
        &state.note_service,
        &state.tag_service,
        &state.search_service,
    )?;
    Ok(status)
}

#[tauri::command]
fn run_pull_only(state: tauri::State<AppState>) -> Result<SyncStatus, String> {
    let status = state
        .sync_service
        .pull_only()
        .map_err(|error| error.to_string())?;

    sync_indexes_from_notes(
        &state.note_service,
        &state.tag_service,
        &state.search_service,
    )?;
    Ok(status)
}

#[tauri::command]
fn run_pull_then_push(state: tauri::State<AppState>) -> Result<SyncStatus, String> {
    let status = state
        .sync_service
        .pull_then_push()
        .map_err(|error| error.to_string())?;

    sync_indexes_from_notes(
        &state.note_service,
        &state.tag_service,
        &state.search_service,
    )?;
    Ok(status)
}

#[tauri::command]
fn run_commit_only(state: tauri::State<AppState>, message: String) -> Result<SyncStatus, String> {
    state
        .sync_service
        .commit_only(&message)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn run_commit_note_only(
    state: tauri::State<AppState>,
    note_id: String,
    message: String,
) -> Result<SyncStatus, String> {
    state
        .sync_service
        .commit_note_only(&note_id, &message)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn note_has_changes(state: tauri::State<AppState>, note_id: String) -> Result<bool, String> {
    state
        .sync_service
        .note_has_changes(&note_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_auto_sync(
    state: tauri::State<AppState>,
    enabled: bool,
    interval_sec: u64,
) -> Result<SyncStatus, String> {
    state
        .sync_service
        .set_auto_sync(enabled, interval_sec)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn resolve_sync_conflict(
    state: tauri::State<AppState>,
    note_id: String,
    use_local: bool,
) -> Result<SyncStatus, String> {
    let status = state
        .sync_service
        .resolve_conflict(&note_id, use_local)
        .map_err(|error| error.to_string())?;

    sync_indexes_from_notes(
        &state.note_service,
        &state.tag_service,
        &state.search_service,
    )?;
    Ok(status)
}

#[tauri::command]
fn finalize_sync_conflicts(state: tauri::State<AppState>) -> Result<SyncStatus, String> {
    let status = state
        .sync_service
        .finalize_conflicts()
        .map_err(|error| error.to_string())?;

    sync_indexes_from_notes(
        &state.note_service,
        &state.tag_service,
        &state.search_service,
    )?;
    Ok(status)
}

#[tauri::command]
fn list_plugins(state: tauri::State<AppState>) -> Result<Vec<PluginEntry>, String> {
    state
        .plugin_service
        .list_plugins()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn install_plugin(
    state: tauri::State<AppState>,
    input: InstallPluginInput,
) -> Result<PluginEntry, String> {
    state
        .plugin_service
        .install_plugin(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_plugin_enabled(
    state: tauri::State<AppState>,
    plugin_id: String,
    enabled: bool,
) -> Result<PluginEntry, String> {
    state
        .plugin_service
        .set_enabled(&plugin_id, enabled)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_plugin(state: tauri::State<AppState>, plugin_id: String) -> Result<(), String> {
    state
        .plugin_service
        .remove_plugin(&plugin_id)
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
        .plugin(tauri_plugin_dialog::init())
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
            search_notes,
            export_note_typst,
            write_export_file,
            compile_typst_to_pdf,
            convert_markdown_text_to_typst,
            save_note_image_attachment,
            load_note_image_attachment,
            save_note_attachment,
            load_note_attachment,
            get_tag_bridge_path,
            get_sync_config_path,
            get_plugin_config_path,
            list_tags,
            list_note_ids_for_tag,
            import_tag_bridge,
            rebuild_tag_index,
            get_sync_status,
            init_sync_repository,
            set_sync_remote_url,
            run_sync_now,
            run_pull_only,
            run_pull_then_push,
            run_commit_only,
            run_commit_note_only,
            note_has_changes,
            set_auto_sync,
            resolve_sync_conflict,
            finalize_sync_conflicts,
            list_plugins,
            install_plugin,
            set_plugin_enabled,
            remove_plugin
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
