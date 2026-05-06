use super::tags::normalize_tag_list;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

const NOTE_FILE_NAME: &str = "note.toml";

#[derive(Debug, Error)]
pub enum NoteError {
    #[error("failed to access note storage: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse note TOML: {0}")]
    ParseToml(#[from] toml::de::Error),
    #[error("failed to serialize note TOML: {0}")]
    SerializeToml(#[from] toml::ser::Error),
    #[error("invalid note path: {0}")]
    InvalidPath(String),
    #[error("note not found: {0}")]
    NoteNotFound(String),
    #[error("folder not found: {0}")]
    FolderNotFound(String),
    #[error("folder already exists: {0}")]
    FolderAlreadyExists(String),
    #[error("note title cannot be empty")]
    EmptyTitle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl NoteBlock {
    fn default_typst() -> Self {
        Self {
            block_type: "typst".to_string(),
            content: Some("= New Note\n\nStart writing here.".to_string()),
            path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteDocument {
    pub title: String,
    pub date: String,
    #[serde(rename = "type")]
    pub note_type: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub content: Vec<NoteBlock>,
}

impl NoteDocument {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.trim().to_string(),
            date: current_date(),
            note_type: "notes".to_string(),
            tags: Vec::new(),
            content: vec![NoteBlock::default_typst()],
        }
    }

    fn normalize(mut self) -> Result<Self, NoteError> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(NoteError::EmptyTitle);
        }
        self.title = title.to_string();

        if self.date.trim().is_empty() {
            self.date = current_date();
        }

        if self.note_type.trim().is_empty() {
            self.note_type = "notes".to_string();
        }

        self.tags = normalize_tag_list(self.tags);

        if self.content.is_empty() {
            self.content.push(NoteBlock::default_typst());
        }

        self.content = self
            .content
            .into_iter()
            .map(|block| NoteBlock {
                block_type: if block.block_type.trim().is_empty() {
                    "typst".to_string()
                } else {
                    block.block_type.trim().to_string()
                },
                content: block.content,
                path: block.path,
            })
            .collect();

        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRecord {
    pub id: String,
    pub folder: String,
    pub document: NoteDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: String,
    pub folder: String,
    pub title: String,
    pub date: String,
    #[serde(rename = "type")]
    pub note_type: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderEntry {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NoteTree {
    pub folders: Vec<FolderEntry>,
    pub notes: Vec<NoteSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveNoteInput {
    pub id: String,
    pub document: NoteDocument,
}

#[derive(Debug, Clone)]
pub struct NoteService {
    root_dir: PathBuf,
}

impl NoteService {
    pub fn new(root_dir: impl Into<PathBuf>) -> Result<Self, NoteError> {
        let root_dir = root_dir.into();
        fs::create_dir_all(&root_dir)?;
        Ok(Self { root_dir })
    }

    pub fn list_tree(&self) -> Result<NoteTree, NoteError> {
        let mut tree = NoteTree::default();
        self.collect_tree(Path::new(""), &mut tree)?;

        tree.folders
            .sort_by(|left, right| left.path.cmp(&right.path));
        tree.notes
            .sort_by(|left, right| left.title.cmp(&right.title));

        Ok(tree)
    }

    pub fn create_folder(&self, parent: &str, name: &str) -> Result<FolderEntry, NoteError> {
        let clean_name = sanitize_path_segment(name)
            .ok_or_else(|| NoteError::InvalidPath(name.trim().to_string()))?;
        let parent_path = normalize_relative_path(parent)?;

        let folder_path = if parent_path.as_os_str().is_empty() {
            PathBuf::from(&clean_name)
        } else {
            parent_path.join(&clean_name)
        };

        let absolute = self.resolve_relative_path(&folder_path)?;
        fs::create_dir_all(absolute)?;

        Ok(FolderEntry {
            path: path_to_string(&folder_path),
            name: clean_name,
        })
    }

    pub fn create_note(&self, folder: &str, title: &str) -> Result<NoteRecord, NoteError> {
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return Err(NoteError::EmptyTitle);
        }

        let folder_path = normalize_relative_path(folder)?;
        let folder_absolute = self.resolve_relative_path(&folder_path)?;
        fs::create_dir_all(&folder_absolute)?;

        let note_folder_name = self.next_note_folder_name(clean_title, &folder_absolute);
        let note_path = if folder_path.as_os_str().is_empty() {
            PathBuf::from(&note_folder_name)
        } else {
            folder_path.join(&note_folder_name)
        };

        let note_absolute = self.resolve_relative_path(&note_path)?;
        fs::create_dir_all(&note_absolute)?;

        let document = NoteDocument::new(clean_title);
        self.write_note_document(&note_absolute, &document)?;

        Ok(NoteRecord {
            id: path_to_string(&note_path),
            folder: path_to_string(&folder_path),
            document,
        })
    }

    pub fn rename_folder(&self, path: &str, name: &str) -> Result<FolderEntry, NoteError> {
        let folder_path = normalize_relative_path(path)?;
        if folder_path.as_os_str().is_empty() {
            return Err(NoteError::InvalidPath(path.to_string()));
        }

        let clean_name = sanitize_path_segment(name)
            .ok_or_else(|| NoteError::InvalidPath(name.trim().to_string()))?;

        let source_absolute = self.resolve_relative_path(&folder_path)?;
        if !source_absolute.is_dir() {
            return Err(NoteError::FolderNotFound(path_to_string(&folder_path)));
        }

        let parent = folder_path.parent().unwrap_or(Path::new(""));
        let next_path = if parent.as_os_str().is_empty() {
            PathBuf::from(&clean_name)
        } else {
            parent.join(&clean_name)
        };

        if next_path == folder_path {
            return Ok(FolderEntry {
                path: path_to_string(&folder_path),
                name: clean_name,
            });
        }

        let destination_absolute = self.resolve_relative_path(&next_path)?;
        if destination_absolute.exists() {
            return Err(NoteError::FolderAlreadyExists(path_to_string(&next_path)));
        }

        fs::rename(source_absolute, destination_absolute)?;

        Ok(FolderEntry {
            path: path_to_string(&next_path),
            name: clean_name,
        })
    }

    pub fn load_note(&self, note_id: &str) -> Result<NoteRecord, NoteError> {
        let note_path = normalize_relative_path(note_id)?;
        if note_path.as_os_str().is_empty() {
            return Err(NoteError::InvalidPath(note_id.to_string()));
        }

        self.read_note_record(&note_path)
    }

    pub fn save_note(&self, input: SaveNoteInput) -> Result<NoteRecord, NoteError> {
        let note_path = normalize_relative_path(&input.id)?;
        if note_path.as_os_str().is_empty() {
            return Err(NoteError::InvalidPath(input.id));
        }

        let note_absolute = self.resolve_relative_path(&note_path)?;
        if !note_absolute.join(NOTE_FILE_NAME).exists() {
            return Err(NoteError::NoteNotFound(path_to_string(&note_path)));
        }

        let document = input.document.normalize()?;
        self.write_note_document(&note_absolute, &document)?;

        let folder = note_path.parent().map(path_to_string).unwrap_or_default();

        Ok(NoteRecord {
            id: path_to_string(&note_path),
            folder,
            document,
        })
    }

    pub fn delete_note(&self, note_id: &str) -> Result<(), NoteError> {
        let note_path = normalize_relative_path(note_id)?;
        if note_path.as_os_str().is_empty() {
            return Err(NoteError::InvalidPath(note_id.to_string()));
        }

        let note_absolute = self.resolve_relative_path(&note_path)?;
        if !note_absolute.join(NOTE_FILE_NAME).is_file() {
            return Err(NoteError::NoteNotFound(path_to_string(&note_path)));
        }

        fs::remove_dir_all(note_absolute)?;
        Ok(())
    }

    pub fn delete_folder(&self, path: &str) -> Result<(), NoteError> {
        let folder_path = normalize_relative_path(path)?;
        if folder_path.as_os_str().is_empty() {
            return Err(NoteError::InvalidPath(path.to_string()));
        }

        let folder_absolute = self.resolve_relative_path(&folder_path)?;
        if !folder_absolute.is_dir() {
            return Err(NoteError::FolderNotFound(path_to_string(&folder_path)));
        }

        fs::remove_dir_all(folder_absolute)?;
        Ok(())
    }

    fn collect_tree(&self, relative: &Path, tree: &mut NoteTree) -> Result<(), NoteError> {
        let current = self.resolve_relative_path(relative)?;

        if current.join(NOTE_FILE_NAME).is_file() {
            let note = self.read_note_record(relative)?;
            tree.notes.push(NoteSummary {
                id: note.id,
                folder: note.folder,
                title: note.document.title,
                date: note.document.date,
                note_type: note.document.note_type,
                tags: note.document.tags,
            });
            return Ok(());
        }

        if !relative.as_os_str().is_empty() {
            tree.folders.push(FolderEntry {
                path: path_to_string(relative),
                name: relative
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
                    .to_string(),
            });
        }

        let mut sub_directories = Vec::new();
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name();
                let is_hidden = name
                    .to_str()
                    .map(|segment| segment.starts_with('.'))
                    .unwrap_or(false);

                if is_hidden {
                    continue;
                }

                sub_directories.push(name);
            }
        }

        sub_directories.sort();

        for directory_name in sub_directories {
            self.collect_tree(&relative.join(directory_name), tree)?;
        }

        Ok(())
    }

    fn read_note_record(&self, note_path: &Path) -> Result<NoteRecord, NoteError> {
        let note_absolute = self.resolve_relative_path(note_path)?;
        let note_file = note_absolute.join(NOTE_FILE_NAME);

        if !note_file.is_file() {
            return Err(NoteError::NoteNotFound(path_to_string(note_path)));
        }

        let raw = fs::read_to_string(note_file)?;
        let document: NoteDocument = toml::from_str(&raw)?;
        let document = document.normalize()?;

        let folder = note_path.parent().map(path_to_string).unwrap_or_default();

        Ok(NoteRecord {
            id: path_to_string(note_path),
            folder,
            document,
        })
    }

    fn write_note_document(
        &self,
        note_absolute: &Path,
        document: &NoteDocument,
    ) -> Result<(), NoteError> {
        let serialized = toml::to_string_pretty(document)?;
        fs::write(note_absolute.join(NOTE_FILE_NAME), serialized)?;
        Ok(())
    }

    fn resolve_relative_path(&self, relative: &Path) -> Result<PathBuf, NoteError> {
        let mut clean = PathBuf::new();

        for component in relative.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(segment) => clean.push(segment),
                _ => return Err(NoteError::InvalidPath(path_to_string(relative))),
            }
        }

        Ok(self.root_dir.join(clean))
    }

    fn next_note_folder_name(&self, title: &str, folder_absolute: &Path) -> String {
        let slug = slugify_title(title);
        let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();
        let base = format!("{}-{}", slug, timestamp);

        if !folder_absolute.join(&base).exists() {
            return base;
        }

        for suffix in 1..1000 {
            let candidate = format!("{}-{}", base, suffix);
            if !folder_absolute.join(&candidate).exists() {
                return candidate;
            }
        }

        format!("{}-fallback", base)
    }
}

fn current_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn normalize_relative_path(raw: &str) -> Result<PathBuf, NoteError> {
    let compact = raw.trim().replace('\\', "/");
    if compact.is_empty() {
        return Ok(PathBuf::new());
    }

    let mut clean = PathBuf::new();
    for component in Path::new(&compact).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => clean.push(segment),
            _ => return Err(NoteError::InvalidPath(raw.to_string())),
        }
    }

    Ok(clean)
}

fn path_to_string(path: &Path) -> String {
    path.iter()
        .map(|segment| segment.to_string_lossy().into_owned())
        .collect::<Vec<String>>()
        .join("/")
}

fn sanitize_path_segment(raw: &str) -> Option<String> {
    let mut clean = String::new();

    for character in raw.trim().chars() {
        if character.is_ascii_alphanumeric() {
            clean.push(character.to_ascii_lowercase());
            continue;
        }

        if matches!(character, '-' | '_') {
            clean.push(character);
            continue;
        }

        if character.is_whitespace() {
            clean.push('-');
        }
    }

    while clean.contains("--") {
        clean = clean.replace("--", "-");
    }

    let clean = clean.trim_matches('-').to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn slugify_title(raw: &str) -> String {
    sanitize_path_segment(raw).unwrap_or_else(|| "note".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn note_toml_roundtrip_supports_inline_and_external_blocks() {
        let document = NoteDocument {
            title: "My Note".to_string(),
            date: "2023-01-01".to_string(),
            note_type: "notes".to_string(),
            tags: vec!["work/project/tasks".to_string()],
            content: vec![
                NoteBlock {
                    block_type: "typst".to_string(),
                    content: Some("= Hello".to_string()),
                    path: None,
                },
                NoteBlock {
                    block_type: "canvas".to_string(),
                    content: None,
                    path: Some("./draw.svg".to_string()),
                },
            ],
        };

        let toml_raw = toml::to_string_pretty(&document).expect("serialize to toml");
        let parsed: NoteDocument = toml::from_str(&toml_raw).expect("parse from toml");

        assert_eq!(document, parsed);
    }

    #[test]
    fn service_creates_loads_and_saves_notes() {
        let temp = tempdir().expect("create tempdir");
        let service = NoteService::new(temp.path()).expect("create service");

        let folder = service
            .create_folder("", "Work")
            .expect("create folder entry");
        let created = service
            .create_note(&folder.path, "Weekly Plan")
            .expect("create note");

        let mut updated_document = created.document.clone();
        updated_document.tags = vec!["work/project/tasks".to_string()];
        updated_document.content.push(NoteBlock {
            block_type: "canvas".to_string(),
            content: None,
            path: Some("./draw.svg".to_string()),
        });

        let saved = service
            .save_note(SaveNoteInput {
                id: created.id.clone(),
                document: updated_document,
            })
            .expect("save note");

        assert_eq!(saved.document.tags, vec!["work/project/tasks".to_string()]);
        assert_eq!(saved.document.content.len(), 2);

        let loaded = service.load_note(&created.id).expect("load note");
        assert_eq!(loaded.document.title, "Weekly Plan");
        assert_eq!(loaded.document.content.len(), 2);

        service.delete_note(&created.id).expect("delete note");
        let deleted_result = service.load_note(&created.id);
        assert!(deleted_result.is_err());
    }

    #[test]
    fn service_lists_folders_and_notes() {
        let temp = tempdir().expect("create tempdir");
        let service = NoteService::new(temp.path()).expect("create service");

        service
            .create_folder("", "Work")
            .expect("create root folder");
        service
            .create_folder("work", "project")
            .expect("create nested folder");
        service
            .create_note("work/project", "Sprint")
            .expect("create nested note");

        let tree = service.list_tree().expect("list tree");

        assert_eq!(tree.folders.len(), 2);
        assert_eq!(tree.notes.len(), 1);
        assert_eq!(tree.notes[0].folder, "work/project");
    }

    #[test]
    fn relative_path_normalization_rejects_parent_segments() {
        let normalized = normalize_relative_path("work/../secret");
        assert!(normalized.is_err());
    }

    #[test]
    fn service_renames_and_deletes_folder() {
        let temp = tempdir().expect("create tempdir");
        let service = NoteService::new(temp.path()).expect("create service");

        service
            .create_folder("", "Work")
            .expect("create root folder");
        service
            .create_folder("work", "project")
            .expect("create nested folder");
        let created = service
            .create_note("work/project", "Draft")
            .expect("create note");

        let renamed = service
            .rename_folder("work/project", "archive")
            .expect("rename folder");
        assert_eq!(renamed.path, "work/archive");

        let tree_after_rename = service.list_tree().expect("list tree after rename");
        assert!(tree_after_rename
            .folders
            .iter()
            .any(|folder| folder.path == "work/archive"));
        assert!(tree_after_rename
            .notes
            .iter()
            .any(|note| note.folder == "work/archive"));

        let renamed_note_id = created.id.replacen("work/project", "work/archive", 1);
        let loaded = service
            .load_note(&renamed_note_id)
            .expect("load moved note");
        assert_eq!(loaded.document.title, "Draft");

        service
            .delete_folder("work/archive")
            .expect("delete folder");

        let tree_after_delete = service.list_tree().expect("list tree after folder delete");
        assert!(tree_after_delete
            .folders
            .iter()
            .all(|folder| folder.path != "work/archive"));
        assert!(tree_after_delete.notes.is_empty());
    }
}
