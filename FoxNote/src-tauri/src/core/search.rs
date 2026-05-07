use super::notes::{NoteDocument, NoteRecord, NoteService};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tantivy::{
    collector::TopDocs,
    query::{AllQuery, Query, QueryParser},
    schema::{Field, Schema, SchemaBuilder, Value, STORED, STRING, TEXT},
    Index, TantivyDocument, Term,
};
use thiserror::Error;

const FIELD_NOTE_ID: &str = "note_id";
const FIELD_TITLE: &str = "title";
const FIELD_FOLDER: &str = "folder";
const FIELD_DATE: &str = "date";
const FIELD_NOTE_TYPE: &str = "note_type";
const FIELD_CONTENT: &str = "content";
const FIELD_SNIPPET: &str = "snippet";
const FIELD_TAG: &str = "tag";

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("failed to access search storage: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to access search index: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
    #[error("failed to access notes while indexing: {0}")]
    Note(#[from] super::notes::NoteError),
    #[error("search index schema missing field: {0}")]
    MissingField(&'static str),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSearchHit {
    pub id: String,
    pub folder: String,
    pub title: String,
    pub date: String,
    #[serde(rename = "type")]
    pub note_type: String,
    pub tags: Vec<String>,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct SearchService {
    index_dir: PathBuf,
}

#[derive(Debug, Clone, Copy)]
struct SearchFields {
    note_id: Field,
    title: Field,
    folder: Field,
    date: Field,
    note_type: Field,
    content: Field,
    snippet: Field,
    tag: Field,
}

impl SearchFields {
    fn from_schema(schema: &Schema) -> Result<Self, SearchError> {
        Ok(Self {
            note_id: schema
                .get_field(FIELD_NOTE_ID)
                .map_err(|_| SearchError::MissingField(FIELD_NOTE_ID))?,
            title: schema
                .get_field(FIELD_TITLE)
                .map_err(|_| SearchError::MissingField(FIELD_TITLE))?,
            folder: schema
                .get_field(FIELD_FOLDER)
                .map_err(|_| SearchError::MissingField(FIELD_FOLDER))?,
            date: schema
                .get_field(FIELD_DATE)
                .map_err(|_| SearchError::MissingField(FIELD_DATE))?,
            note_type: schema
                .get_field(FIELD_NOTE_TYPE)
                .map_err(|_| SearchError::MissingField(FIELD_NOTE_TYPE))?,
            content: schema
                .get_field(FIELD_CONTENT)
                .map_err(|_| SearchError::MissingField(FIELD_CONTENT))?,
            snippet: schema
                .get_field(FIELD_SNIPPET)
                .map_err(|_| SearchError::MissingField(FIELD_SNIPPET))?,
            tag: schema
                .get_field(FIELD_TAG)
                .map_err(|_| SearchError::MissingField(FIELD_TAG))?,
        })
    }
}

impl SearchService {
    pub fn new(index_dir: impl Into<PathBuf>) -> Result<Self, SearchError> {
        let service = Self {
            index_dir: index_dir.into(),
        };

        let _ = service.open_or_create_index_with_fields()?;
        Ok(service)
    }

    pub fn rebuild_from_notes(&self, note_service: &NoteService) -> Result<(), SearchError> {
        let (index, fields) = self.recreate_index_with_fields()?;
        let mut writer = index.writer::<TantivyDocument>(50_000_000)?;

        let tree = note_service.list_tree()?;
        for summary in tree.notes {
            let record = note_service.load_note(&summary.id)?;
            Self::index_record(&mut writer, &fields, &record);
        }

        writer.commit()?;
        Ok(())
    }

    pub fn upsert_note_record(&self, record: &NoteRecord) -> Result<(), SearchError> {
        let (index, fields) = self.open_or_create_index_with_fields()?;
        let mut writer = index.writer::<TantivyDocument>(20_000_000)?;
        writer.delete_term(Term::from_field_text(fields.note_id, &record.id));
        Self::index_record(&mut writer, &fields, record);
        writer.commit()?;
        Ok(())
    }

    pub fn remove_note(&self, note_id: &str) -> Result<(), SearchError> {
        let (index, fields) = self.open_or_create_index_with_fields()?;
        let mut writer = index.writer::<TantivyDocument>(20_000_000)?;
        writer.delete_term(Term::from_field_text(fields.note_id, note_id));
        writer.commit()?;
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        tag_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<NoteSearchHit>, SearchError> {
        let trimmed_query = query.trim();
        let normalized_tag = tag_filter.map(str::trim).filter(|value| !value.is_empty());
        if trimmed_query.is_empty() && normalized_tag.is_none() {
            return Ok(Vec::new());
        }

        let (index, fields) = self.open_or_create_index_with_fields()?;
        let reader = index.reader()?;
        let searcher = reader.searcher();
        let result_limit = limit.clamp(1, 200);
        let fetch_limit = (result_limit * 6).clamp(20, 1200);

        let boxed_query: Box<dyn Query> = if trimmed_query.is_empty() {
            Box::new(AllQuery)
        } else {
            let parser =
                QueryParser::for_index(&index, vec![fields.title, fields.content, fields.folder]);
            match parser.parse_query(trimmed_query) {
                Ok(parsed) => parsed,
                Err(_) => return Ok(Vec::new()),
            }
        };

        let top_docs = searcher.search(&boxed_query, &TopDocs::with_limit(fetch_limit))?;
        let mut hits = Vec::with_capacity(result_limit);

        for (score, address) in top_docs {
            let document: TantivyDocument = searcher.doc(address)?;
            let tags = document
                .get_all(fields.tag)
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect::<Vec<String>>();

            if let Some(active_tag) = normalized_tag {
                if !matches_tag_filter(&tags, active_tag) {
                    continue;
                }
            }

            hits.push(NoteSearchHit {
                id: field_text(&document, fields.note_id),
                title: field_text(&document, fields.title),
                folder: field_text(&document, fields.folder),
                date: field_text(&document, fields.date),
                note_type: field_text(&document, fields.note_type),
                snippet: field_text(&document, fields.snippet),
                tags,
                score,
            });

            if hits.len() >= result_limit {
                break;
            }
        }

        Ok(hits)
    }

    fn index_record(
        writer: &mut tantivy::IndexWriter<TantivyDocument>,
        fields: &SearchFields,
        record: &NoteRecord,
    ) {
        let (search_text, snippet) = build_search_text(&record.document);

        let mut document = TantivyDocument::default();
        document.add_text(fields.note_id, &record.id);
        document.add_text(fields.title, &record.document.title);
        document.add_text(fields.folder, &record.folder);
        document.add_text(fields.date, &record.document.date);
        document.add_text(fields.note_type, &record.document.note_type);
        document.add_text(fields.content, &search_text);
        document.add_text(fields.snippet, &snippet);

        for tag in &record.document.tags {
            let trimmed = tag.trim();
            if !trimmed.is_empty() {
                document.add_text(fields.tag, trimmed);
            }
        }

        let _ = writer.add_document(document);
    }

    fn open_or_create_index_with_fields(&self) -> Result<(Index, SearchFields), SearchError> {
        let index = self.open_or_create_index()?;
        match SearchFields::from_schema(&index.schema()) {
            Ok(fields) => Ok((index, fields)),
            Err(SearchError::MissingField(_)) => {
                let (recreated, fields) = self.recreate_index_with_fields()?;
                Ok((recreated, fields))
            }
            Err(other) => Err(other),
        }
    }

    fn open_or_create_index(&self) -> Result<Index, SearchError> {
        fs::create_dir_all(&self.index_dir)?;

        let meta_path = self.index_dir.join("meta.json");
        if meta_path.is_file() {
            return Ok(Index::open_in_dir(&self.index_dir)?);
        }

        Ok(Index::create_in_dir(&self.index_dir, build_schema())?)
    }

    fn recreate_index_with_fields(&self) -> Result<(Index, SearchFields), SearchError> {
        reset_directory(&self.index_dir)?;
        let index = Index::create_in_dir(&self.index_dir, build_schema())?;
        let fields = SearchFields::from_schema(&index.schema())?;
        Ok((index, fields))
    }
}

fn build_schema() -> Schema {
    let mut builder = SchemaBuilder::new();
    builder.add_text_field(FIELD_NOTE_ID, STRING | STORED);
    builder.add_text_field(FIELD_TITLE, TEXT | STORED);
    builder.add_text_field(FIELD_FOLDER, TEXT | STORED);
    builder.add_text_field(FIELD_DATE, STRING | STORED);
    builder.add_text_field(FIELD_NOTE_TYPE, STRING | STORED);
    builder.add_text_field(FIELD_CONTENT, TEXT);
    builder.add_text_field(FIELD_SNIPPET, TEXT | STORED);
    builder.add_text_field(FIELD_TAG, STRING | STORED);
    builder.build()
}

fn build_search_text(document: &NoteDocument) -> (String, String) {
    let mut parts = Vec::new();
    if !document.title.trim().is_empty() {
        parts.push(document.title.trim().to_string());
    }

    for block in &document.content {
        if let Some(content) = block
            .content
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            parts.push(content.to_string());
        }

        if let Some(summary) = block
            .summary
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            parts.push(summary.to_string());
        }
    }

    let joined = parts.join("\n\n");
    let snippet = joined
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .chars()
        .take(240)
        .collect::<String>();
    (joined, snippet)
}

fn matches_tag_filter(tags: &[String], active_tag: &str) -> bool {
    let prefix = format!("{active_tag}/");
    tags.iter()
        .any(|tag| tag == active_tag || tag.starts_with(&prefix))
}

fn field_text(document: &TantivyDocument, field: Field) -> String {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn reset_directory(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)
}
