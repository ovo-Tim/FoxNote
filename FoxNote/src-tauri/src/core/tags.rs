use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};
use thiserror::Error;

const EXPORT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum TagIndexError {
    #[error("failed to access tag index storage: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to access sqlite tag index: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("failed to parse tag bridge TOML: {0}")]
    ParseToml(#[from] toml::de::Error),
    #[error("failed to serialize tag bridge TOML: {0}")]
    SerializeToml(#[from] toml::ser::Error),
    #[error("unsupported tag bridge version: {0}")]
    UnsupportedVersion(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TagEntry {
    pub path: String,
    pub name: String,
    pub note_count: u32,
}

#[derive(Debug, Clone)]
pub struct TagIndexService {
    db_path: PathBuf,
    export_path: PathBuf,
}

impl TagIndexService {
    pub fn new(
        db_path: impl Into<PathBuf>,
        export_path: impl Into<PathBuf>,
    ) -> Result<Self, TagIndexError> {
        let service = Self {
            db_path: db_path.into(),
            export_path: export_path.into(),
        };

        service.ensure_storage_dirs()?;
        service.ensure_schema()?;
        if !service.export_path.is_file() {
            service.write_export()?;
        }

        Ok(service)
    }

    pub fn list_tags(&self) -> Result<Vec<TagEntry>, TagIndexError> {
        let connection = self.open_connection()?;

        let mut statement = connection.prepare(
            "
            SELECT t.path, t.name, COUNT(nt.note_id) AS note_count
            FROM tags t
            LEFT JOIN note_tags nt ON nt.tag_path = t.path
            GROUP BY t.path, t.name
            ORDER BY t.path
            ",
        )?;

        let rows = statement.query_map([], |row| {
            let note_count: i64 = row.get(2)?;
            Ok(TagEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                note_count: note_count.max(0) as u32,
            })
        })?;

        let mut tags = Vec::new();
        for row in rows {
            tags.push(row?);
        }
        Ok(tags)
    }

    pub fn note_ids_for_tag(&self, tag_path: &str) -> Result<Vec<String>, TagIndexError> {
        let Some(tag_path) = normalize_tag_path(tag_path) else {
            return Ok(Vec::new());
        };

        let like_pattern = format!("{tag_path}/%");
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "
            SELECT DISTINCT note_id
            FROM note_tags
            WHERE tag_path = ?1 OR tag_path LIKE ?2
            ORDER BY note_id
            ",
        )?;

        let rows = statement.query_map(params![tag_path, like_pattern], |row| row.get(0))?;
        let mut note_ids = Vec::new();
        for row in rows {
            note_ids.push(row?);
        }
        Ok(note_ids)
    }

    pub fn replace_all(&self, note_entries: &[(String, Vec<String>)]) -> Result<(), TagIndexError> {
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;

        transaction.execute("DELETE FROM note_tags", [])?;
        transaction.execute("DELETE FROM tags", [])?;

        for (note_id, tags) in note_entries {
            self.write_note_tags(&transaction, note_id, tags)?;
        }

        transaction.commit()?;
        self.write_export()?;
        Ok(())
    }

    pub fn upsert_note_tags(&self, note_id: &str, tags: &[String]) -> Result<(), TagIndexError> {
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;

        transaction.execute("DELETE FROM note_tags WHERE note_id = ?1", params![note_id])?;
        self.write_note_tags(&transaction, note_id, tags)?;
        Self::purge_orphan_tags(&transaction)?;

        transaction.commit()?;
        self.write_export()?;
        Ok(())
    }

    pub fn remove_note(&self, note_id: &str) -> Result<(), TagIndexError> {
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;

        transaction.execute("DELETE FROM note_tags WHERE note_id = ?1", params![note_id])?;
        Self::purge_orphan_tags(&transaction)?;

        transaction.commit()?;
        self.write_export()?;
        Ok(())
    }

    pub fn import_from_export(&self) -> Result<(), TagIndexError> {
        if !self.export_path.is_file() {
            return Ok(());
        }

        let raw = fs::read_to_string(&self.export_path)?;
        let export: TagBridgeDocument = toml::from_str(&raw)?;
        if export.version != EXPORT_VERSION {
            return Err(TagIndexError::UnsupportedVersion(export.version));
        }

        let mut note_to_tags: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for tag in export.tags {
            let Some(tag_path) = normalize_tag_path(&tag.path) else {
                continue;
            };

            for note_id in tag.notes {
                let note_id = note_id.trim();
                if note_id.is_empty() {
                    continue;
                }
                note_to_tags
                    .entry(note_id.to_string())
                    .or_default()
                    .push(tag_path.clone());
            }
        }

        let records = note_to_tags
            .into_iter()
            .collect::<Vec<(String, Vec<String>)>>();
        self.replace_all(&records)?;
        Ok(())
    }

    fn ensure_storage_dirs(&self) -> Result<(), TagIndexError> {
        if let Some(parent) = self.db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.export_path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    fn ensure_schema(&self) -> Result<(), TagIndexError> {
        let connection = self.open_connection()?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tags (
                path TEXT PRIMARY KEY,
                name TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS note_tags (
                note_id TEXT NOT NULL,
                tag_path TEXT NOT NULL,
                PRIMARY KEY (note_id, tag_path),
                FOREIGN KEY (tag_path) REFERENCES tags(path) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_note_tags_note_id ON note_tags(note_id);
            CREATE INDEX IF NOT EXISTS idx_note_tags_tag_path ON note_tags(tag_path);
            ",
        )?;
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection, TagIndexError> {
        let connection = Connection::open(&self.db_path)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tags (
                path TEXT PRIMARY KEY,
                name TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS note_tags (
                note_id TEXT NOT NULL,
                tag_path TEXT NOT NULL,
                PRIMARY KEY (note_id, tag_path),
                FOREIGN KEY (tag_path) REFERENCES tags(path) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_note_tags_note_id ON note_tags(note_id);
            CREATE INDEX IF NOT EXISTS idx_note_tags_tag_path ON note_tags(tag_path);
            ",
        )?;

        Ok(connection)
    }

    fn write_note_tags(
        &self,
        transaction: &Transaction<'_>,
        note_id: &str,
        tags: &[String],
    ) -> Result<(), TagIndexError> {
        let note_id = note_id.trim();
        if note_id.is_empty() {
            return Ok(());
        }

        for tag_path in normalize_tag_list(tags.to_vec()) {
            transaction.execute(
                "
                INSERT INTO tags (path, name)
                VALUES (?1, ?2)
                ON CONFLICT(path) DO UPDATE SET name = excluded.name
                ",
                params![tag_path, tag_name(&tag_path)],
            )?;

            transaction.execute(
                "INSERT OR IGNORE INTO note_tags (note_id, tag_path) VALUES (?1, ?2)",
                params![note_id, tag_path],
            )?;
        }

        Ok(())
    }

    fn purge_orphan_tags(transaction: &Transaction<'_>) -> Result<(), TagIndexError> {
        transaction.execute(
            "
            DELETE FROM tags
            WHERE path NOT IN (
                SELECT DISTINCT tag_path
                FROM note_tags
            )
            ",
            [],
        )?;
        Ok(())
    }

    fn write_export(&self) -> Result<(), TagIndexError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "
            SELECT t.path, nt.note_id
            FROM tags t
            LEFT JOIN note_tags nt ON nt.tag_path = t.path
            ORDER BY t.path, nt.note_id
            ",
        )?;

        let mut rows = statement.query([])?;
        let mut tags = BTreeMap::<String, BTreeSet<String>>::new();

        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let note_id: Option<String> = row.get(1)?;

            let tag_notes = tags.entry(path).or_default();
            if let Some(note_id) = note_id {
                let note_id = note_id.trim();
                if !note_id.is_empty() {
                    tag_notes.insert(note_id.to_string());
                }
            }
        }

        let document = TagBridgeDocument {
            version: EXPORT_VERSION,
            tags: tags
                .into_iter()
                .map(|(path, notes)| TagBridgeEntry {
                    path,
                    notes: notes.into_iter().collect(),
                })
                .collect(),
        };

        let raw = toml::to_string_pretty(&document)?;
        fs::write(&self.export_path, raw)?;
        Ok(())
    }
}

pub fn normalize_tag_list(tags: Vec<String>) -> Vec<String> {
    let mut normalized = BTreeSet::new();

    for tag in tags {
        if let Some(path) = normalize_tag_path(&tag) {
            normalized.insert(path);
        }
    }

    normalized.into_iter().collect()
}

pub fn normalize_tag_path(raw: &str) -> Option<String> {
    let compact = raw.trim().replace('\\', "/");
    if compact.is_empty() {
        return None;
    }

    let segments = compact
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    }
}

fn tag_name(path: &str) -> String {
    path.split('/').next_back().unwrap_or(path).to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TagBridgeDocument {
    version: u32,
    #[serde(default)]
    tags: Vec<TagBridgeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TagBridgeEntry {
    path: String,
    #[serde(default)]
    notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalize_tag_list_compacts_nested_paths() {
        let tags = normalize_tag_list(vec![
            " work / project / tasks ".to_string(),
            "work/project/tasks".to_string(),
            "dev\\rust".to_string(),
            "".to_string(),
            "////".to_string(),
        ]);

        assert_eq!(
            tags,
            vec!["dev/rust".to_string(), "work/project/tasks".to_string()]
        );
    }

    #[test]
    fn tag_index_rebuilds_exports_and_queries_descendants() {
        let temp = tempdir().expect("create tempdir");
        let db_path = temp.path().join("tags.sqlite3");
        let export_path = temp.path().join("tags-index.toml");

        let service =
            TagIndexService::new(db_path, export_path.clone()).expect("create tag index service");
        service
            .replace_all(&[
                (
                    "work/note-a".to_string(),
                    vec!["work/project/tasks".to_string()],
                ),
                (
                    "work/note-b".to_string(),
                    vec!["work/project".to_string(), "personal/reading".to_string()],
                ),
            ])
            .expect("replace all tag mappings");

        let tags = service.list_tags().expect("list tags");
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].path, "personal/reading");
        assert_eq!(tags[0].note_count, 1);

        let work_notes = service
            .note_ids_for_tag("work")
            .expect("query notes by parent tag");
        assert_eq!(
            work_notes,
            vec!["work/note-a".to_string(), "work/note-b".to_string()]
        );

        let export_raw = fs::read_to_string(export_path).expect("read export toml");
        assert!(export_raw.contains("version = 1"));
        assert!(export_raw.contains("path = \"work/project\""));
    }

    #[test]
    fn tag_bridge_import_roundtrip_restores_mappings() {
        let temp = tempdir().expect("create tempdir");
        let export_path = temp.path().join("tags-index.toml");

        let writer_service = TagIndexService::new(temp.path().join("writer.sqlite3"), &export_path)
            .expect("create writer service");
        writer_service
            .replace_all(&[
                (
                    "archive/note-1".to_string(),
                    vec!["archive/logs".to_string(), "archive/ideas".to_string()],
                ),
                (
                    "archive/note-2".to_string(),
                    vec!["archive/logs".to_string()],
                ),
            ])
            .expect("write source mappings");

        let reader_service = TagIndexService::new(temp.path().join("reader.sqlite3"), &export_path)
            .expect("create reader service");
        reader_service
            .import_from_export()
            .expect("import bridge export");

        let imported_tags = reader_service.list_tags().expect("list imported tags");
        assert_eq!(imported_tags.len(), 2);

        let imported_notes = reader_service
            .note_ids_for_tag("archive/logs")
            .expect("query imported notes");
        assert_eq!(
            imported_notes,
            vec!["archive/note-1".to_string(), "archive/note-2".to_string()]
        );
    }
}
