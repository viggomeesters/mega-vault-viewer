use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, SecondsFormat, Utc};

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, OwnedValue, Schema, TantivyDocument, FAST, INDEXED, STORED, STRING, TEXT,
};
use tantivy::{doc, Index, IndexWriter, Term};
use walkdir::WalkDir;

const PARSER_VERSION: &str = "mvv-core-parser-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultStats {
    pub documents: usize,
    pub links: usize,
    pub vault_size_bytes: i64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexSummary {
    pub scanned: usize,
    pub skipped: usize,
    pub updated: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub errored: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileManifestEntry {
    pub relative_path: String,
    pub kind: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_ns: Option<i64>,
    pub content_hash: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchHit {
    pub id: Option<i64>,
    pub slug: String,
    pub title: String,
    pub filename: String,
    pub stem: String,
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: String,
    pub extension: String,
    pub size_bytes: i64,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentView {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub filename: String,
    pub stem: String,
    pub path: PathBuf,
    pub relative_path: String,
    pub html: String,
    pub frontmatter: Option<serde_json::Value>,
    pub frontmatter_error: Option<String>,
    pub outgoing_links: Vec<String>,
    pub backlinks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileBrowserItem {
    pub id: Option<i64>,
    pub document_id: Option<i64>,
    pub slug: String,
    pub title: String,
    pub filename: String,
    pub relative_path: String,
    pub kind: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: Option<u64>,
    pub created_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderEntry {
    pub path: String,
    pub document_count: usize,
    pub files: Vec<FileBrowserItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DailyNoteEntry {
    pub date: String,
    pub id: Option<i64>,
    pub filename: String,
    pub relative_path: String,
    pub last_updated: Option<String>,
    pub ai_processed_at: Option<String>,
    pub ai_processed_status: DailyNoteProcessedStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DailyNoteProcessedStatus {
    NotTracked,
    Missing,
    Outdated,
    Processed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultGroupEntry {
    pub name: String,
    pub count: usize,
    pub latest_title: String,
    pub latest_relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterRecordCollection {
    pub file: String,
    pub schema: Option<String>,
    pub count: usize,
    pub record_type: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterVaultSummary {
    pub name: String,
    pub promise: String,
    pub human_owned: Vec<String>,
    pub canonical: Vec<String>,
    pub generated: Vec<String>,
    pub record_collections: Vec<StarterRecordCollection>,
    pub total_records: usize,
    pub human_note_count: usize,
    pub generated_view_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct StarterContract {
    name: Option<String>,
    promise: Option<String>,
    #[serde(default)]
    human_owned: Vec<String>,
    #[serde(default)]
    canonical: Vec<String>,
    #[serde(default)]
    generated: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileBrowserSnapshot {
    pub folders: Vec<FolderEntry>,
    pub newest_files: Vec<FileBrowserItem>,
    pub recent_files: Vec<FileBrowserItem>,
    pub daily_notes: Vec<DailyNoteEntry>,
    pub today_items: Vec<FileBrowserItem>,
    pub timeline_items: Vec<FileBrowserItem>,
    pub entities: Vec<VaultGroupEntry>,
    pub projects: Vec<VaultGroupEntry>,
    pub starter_vault: Option<StarterVaultSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultItemView {
    pub document_id: Option<i64>,
    pub slug: String,
    pub title: String,
    pub filename: String,
    pub stem: String,
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: Option<u64>,
    pub html: Option<String>,
    pub formatted: Option<String>,
    pub source: Option<String>,
    pub media_data_url: Option<String>,
    pub media_mime: Option<String>,
    pub preview_message: Option<String>,
    pub frontmatter: Option<serde_json::Value>,
    pub frontmatter_error: Option<String>,
    pub outgoing_links: Vec<String>,
    pub backlinks: Vec<String>,
    pub can_edit_source: bool,
    pub can_open_system: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VaultRuntime {
    root: PathBuf,
    db_path: PathBuf,
    index_dir: PathBuf,
    last_summary: IndexSummary,
}

#[derive(Debug, Clone)]
struct IndexedDocument {
    slug: String,
    title: String,
    filename: String,
    stem: String,
    path: PathBuf,
    relative_path: String,
    body: String,
    frontmatter: Option<serde_json::Value>,
    frontmatter_error: Option<String>,
    links: Vec<WikiLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WikiLink {
    target: String,
    label: String,
}

#[derive(Debug, Clone)]
struct SearchFields {
    id: Field,
    doc_key: Field,
    slug: Field,
    title: Field,
    filename: Field,
    relative_path: Field,
    body: Field,
}

#[derive(Debug, Clone)]
struct DocumentSummary {
    id: i64,
    slug: String,
    title: String,
    filename: String,
    stem: String,
    path: PathBuf,
    relative_path: String,
    body: String,
}

#[derive(Debug, Clone)]
struct FileIndexState {
    content_hash: String,
    search_hash: Option<String>,
    parser_version: String,
    status: String,
    kind: String,
    extension: String,
}

#[derive(Debug, Clone)]
struct SearchDocumentRow {
    id: i64,
    slug: String,
    title: String,
    filename: String,
    relative_path: String,
    body: String,
}

#[derive(Debug, Clone)]
struct FileRow {
    relative_path: String,
    absolute_path: PathBuf,
    kind: String,
    extension: String,
    size_bytes: i64,
    modified_ns: Option<i64>,
}

#[derive(Debug, Clone)]
struct BrowserFileRow {
    id: Option<i64>,
    slug: String,
    title: Option<String>,
    path: PathBuf,
    relative_path: String,
    kind: String,
    extension: String,
    size_bytes: i64,
    modified_ns: Option<i64>,
}

#[derive(Debug, Clone)]
struct DiscoveredFile {
    path: PathBuf,
    relative_path: String,
    kind: String,
    extension: String,
    size_bytes: i64,
    modified_ns: Option<i64>,
    content_hash: String,
}

impl VaultRuntime {
    pub fn build(root: impl AsRef<Path>, state_dir: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let state_dir = state_dir.as_ref().to_path_buf();
        fs::create_dir_all(&state_dir).context("create state directory")?;

        let db_path = state_dir.join("mega-vault-viewer.sqlite");
        let index_dir = state_dir.join("tantivy");
        fs::create_dir_all(&index_dir).context("create tantivy index directory")?;

        let mut runtime = Self {
            root,
            db_path,
            index_dir,
            last_summary: IndexSummary::default(),
        };
        runtime.last_summary = runtime.sync()?;
        Ok(runtime)
    }

    pub fn reset_runtime_state(state_dir: impl AsRef<Path>) -> Result<()> {
        reset_runtime_state_dir(state_dir.as_ref())
    }

    pub fn stats(&self) -> Result<VaultStats> {
        let conn = self.open_db()?;
        let documents = conn.query_row("select count(*) from documents", [], |row| row.get(0))?;
        let links = conn.query_row("select count(*) from links", [], |row| row.get(0))?;
        let vault_size_bytes = conn.query_row(
            "select coalesce(sum(size_bytes), 0) from files where status = 'indexed'",
            [],
            |row| row.get(0),
        )?;
        Ok(VaultStats {
            documents,
            links,
            vault_size_bytes,
        })
    }

    pub fn index_summary(&self) -> IndexSummary {
        self.last_summary.clone()
    }

    pub fn file_manifest(&self) -> Result<Vec<FileManifestEntry>> {
        let conn = self.open_db()?;
        let mut statement = conn.prepare(
            "select relative_path, kind, extension, size_bytes, modified_ns, content_hash, status from files order by relative_path",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(FileManifestEntry {
                relative_path: row.get(0)?,
                kind: row.get(1)?,
                extension: row.get(2)?,
                size_bytes: row.get(3)?,
                modified_ns: row.get(4)?,
                content_hash: row.get(5)?,
                status: row.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let (index, fields) = self.open_search_index()?;
        let reader = index.reader().context("open tantivy reader")?;
        let searcher = reader.searcher();
        let parser = QueryParser::for_index(
            &index,
            vec![
                fields.title,
                fields.body,
                fields.slug,
                fields.filename,
                fields.relative_path,
            ],
        );
        let file_search_query = query.trim().to_ascii_lowercase();
        let query = parser.parse_query(query).context("parse search query")?;
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .context("search tantivy index")?;

        let conn = self.open_db()?;
        let mut hits = Vec::new();
        for (score, address) in top_docs {
            let retrieved = searcher.doc::<TantivyDocument>(address)?;
            let relative_path = first_text(&retrieved, fields.doc_key);
            let Some(id) = first_u64(&retrieved, fields.id) else {
                continue;
            };
            if let Some(summary) = document_summary(&conn, id as i64)? {
                if relative_path.as_deref() == Some(summary.relative_path.as_str()) {
                    hits.push(SearchHit {
                        id: Some(summary.id),
                        slug: summary.slug,
                        title: summary.title,
                        filename: summary.filename,
                        stem: summary.stem,
                        path: summary.path,
                        relative_path: summary.relative_path,
                        kind: "markdown".to_string(),
                        extension: "md".to_string(),
                        size_bytes: 0,
                        snippet: snippet_for(&summary.body),
                        score,
                    });
                    continue;
                }
            }
            if let Some(relative_path) = relative_path {
                if let Some(file) = file_row_by_relative_path(&conn, &relative_path)? {
                    let path = self.canonical_vault_file_path(&file.absolute_path)?;
                    let body =
                        searchable_structured_source(&path, &file.extension).unwrap_or_default();
                    hits.push(SearchHit {
                        id: None,
                        slug: relative_path.clone(),
                        title: fallback_title(&path),
                        filename: filename_for(&path),
                        stem: stem_for(&path),
                        path,
                        relative_path,
                        kind: file.kind,
                        extension: file.extension,
                        size_bytes: file.size_bytes,
                        snippet: snippet_for(&body),
                        score,
                    });
                }
            }
        }
        append_file_search_hits(&conn, &mut hits, &file_search_query, limit)?;
        hits.truncate(limit);
        Ok(hits)
    }

    pub fn open_by_slug(&self, slug: &str) -> Result<DocumentView> {
        let conn = self.open_db()?;
        let doc = conn
            .query_row(
                "select id, slug, title, filename, stem, path, relative_path, body, frontmatter_json, frontmatter_error from documents where slug = ?1 order by relative_path limit 1",
                [slug],
                |row| row_to_document_view(row, &self.root),
            )
            .optional()?
            .with_context(|| format!("document not found: {slug}"))?;

        self.with_link_context(&conn, doc)
    }

    pub fn open_by_id(&self, id: i64) -> Result<DocumentView> {
        let conn = self.open_db()?;
        let doc = conn
            .query_row(
                "select id, slug, title, filename, stem, path, relative_path, body, frontmatter_json, frontmatter_error from documents where id = ?1",
                [id],
                |row| row_to_document_view(row, &self.root),
            )
            .optional()?
            .with_context(|| format!("document not found: {id}"))?;

        self.with_link_context(&conn, doc)
    }

    pub fn open_by_relative_path(&self, relative_path: &str) -> Result<DocumentView> {
        let conn = self.open_db()?;
        let doc = conn
            .query_row(
                "select id, slug, title, filename, stem, path, relative_path, body, frontmatter_json, frontmatter_error from documents where relative_path = ?1",
                [relative_path],
                |row| row_to_document_view(row, &self.root),
            )
            .optional()?
            .with_context(|| format!("document not found: {relative_path}"))?;

        self.with_link_context(&conn, doc)
    }

    fn with_link_context(&self, conn: &Connection, doc: DocumentView) -> Result<DocumentView> {
        let outgoing_links = collect_strings(
            conn,
            "select target_slug from links where source_slug = ?1 order by target_slug",
            &doc.slug,
        )?;
        let backlinks = collect_strings(
            conn,
            "select source_slug from links where target_slug = ?1 order by source_slug",
            &doc.slug,
        )?;

        Ok(DocumentView {
            outgoing_links,
            backlinks,
            ..doc
        })
    }

    pub fn first_document(&self) -> Result<Option<DocumentView>> {
        let conn = self.open_db()?;
        let slug = conn
            .query_row(
                "select slug from documents order by slug limit 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        slug.map(|slug| self.open_by_slug(&slug)).transpose()
    }

    pub fn file_browser(&self) -> Result<FileBrowserSnapshot> {
        let conn = self.open_db()?;
        let mut statement = conn.prepare(
            r#"
            select d.id, d.slug, d.title, f.absolute_path, f.relative_path, f.kind, f.extension, f.size_bytes, f.modified_ns
            from files f
            left join documents d on d.relative_path = f.relative_path
            where f.status = 'indexed'
            order by f.relative_path
            "#,
        )?;
        let rows = statement.query_map([], |row| {
            Ok(BrowserFileRow {
                id: row.get(0)?,
                slug: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                title: row.get(2)?,
                path: PathBuf::from(row.get::<_, String>(3)?),
                relative_path: row.get(4)?,
                kind: row.get(5)?,
                extension: row.get(6)?,
                size_bytes: row.get(7)?,
                modified_ns: row.get(8)?,
            })
        })?;

        let browser_rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let files = browser_rows
            .iter()
            .cloned()
            .map(file_browser_item)
            .collect::<Vec<_>>();

        let folders = folder_entries(&files);
        let daily_notes = daily_note_entries(&browser_rows);
        let today_items = today_items(&files);
        let timeline_items = timeline_items(&files);
        let entities = grouped_metadata_entries(&conn, GroupKind::Entity)?;
        let projects = grouped_metadata_entries(&conn, GroupKind::Project)?;
        let starter_vault = starter_vault_summary(&self.root, &files)?;
        let mut newest_files = files.clone();
        newest_files.sort_by_key(|file| std::cmp::Reverse(file.created_at.unwrap_or(0)));
        newest_files.truncate(40);

        let mut recent_files = files;
        recent_files.sort_by_key(|file| std::cmp::Reverse(file.modified_at.unwrap_or(0)));
        recent_files.truncate(40);

        Ok(FileBrowserSnapshot {
            folders,
            newest_files,
            recent_files,
            daily_notes,
            today_items,
            timeline_items,
            entities,
            projects,
            starter_vault,
        })
    }

    pub fn open_item_by_relative_path(&self, relative_path: &str) -> Result<VaultItemView> {
        let conn = self.open_db()?;
        let file = file_row_by_relative_path(&conn, relative_path)?
            .with_context(|| format!("file not found: {relative_path}"))?;
        let path = self.canonical_vault_file_path(&file.absolute_path)?;

        if file.kind == "markdown" {
            let can_edit_source = can_edit_markdown_source(&self.root, relative_path);
            return match self.open_by_relative_path(relative_path) {
                Ok(document) => Ok(vault_item_from_document(document, &file, can_edit_source)),
                Err(error) => Ok(vault_item_error(
                    &file,
                    &path,
                    format!("Markdown render failed: {error}"),
                )),
            };
        }

        Ok(match file.kind.as_str() {
            "yaml" | "json" => self.open_text_item(&file, &path),
            "image" => open_image_item(&file, &path),
            "pdf" => vault_item_preview(&file, &path, "PDF preview is not available yet."),
            _ => open_generic_item(&file, &path),
        })
    }

    pub fn open_item_by_slug(&self, slug: &str) -> Result<VaultItemView> {
        let document = self.open_by_slug(slug)?;
        self.open_item_by_relative_path(&document.relative_path)
    }

    pub fn open_item_by_id(&self, id: i64) -> Result<VaultItemView> {
        let document = self.open_by_id(id)?;
        self.open_item_by_relative_path(&document.relative_path)
    }

    pub fn first_item(&self) -> Result<Option<VaultItemView>> {
        let conn = self.open_db()?;
        let relative_path = conn
            .query_row(
                r#"
                select relative_path from files
                where status = 'indexed'
                  and kind = 'markdown'
                  and relative_path glob 'daily/????-??-??.md'
                order by relative_path desc
                limit 1
                "#,
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let relative_path = match relative_path {
            Some(path) => Some(path),
            None => conn
                .query_row(
                    "select relative_path from files where status = 'indexed' order by relative_path limit 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?,
        };
        relative_path
            .map(|relative_path| self.open_item_by_relative_path(&relative_path))
            .transpose()
    }

    pub fn document_source_by_relative_path(&self, relative_path: &str) -> Result<String> {
        let conn = self.open_db()?;
        let path = self.document_path_by_relative_path(&conn, relative_path)?;
        let path = self.canonical_document_path(&path)?;
        fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))
    }

    pub fn ensure_daily_note(&self, date: &str) -> Result<String> {
        if !is_valid_daily_date(date) {
            anyhow::bail!("invalid daily note date: {date}");
        }
        let relative_path = format!("daily/{date}.md");
        let path = self.root.join(&relative_path);
        let path = canonical_path_for_new_file(&self.root, &path)?;
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            let now = iso_timestamp_now();
            let source =
                format!("---\ntitle: {date}\ndate: {date}\nlast_updated: {now}\n---\n\n# {date}\n");
            fs::write(&path, source).with_context(|| format!("write {}", path.display()))?;
        }
        Ok(relative_path)
    }

    pub fn write_document_source_by_relative_path(
        &self,
        relative_path: &str,
        source: &str,
    ) -> Result<()> {
        if !can_edit_markdown_source(&self.root, relative_path) {
            anyhow::bail!(
                "document is protected by the starter vault human-owned contract: {relative_path}"
            );
        }
        let conn = self.open_db()?;
        let path = self.document_path_by_relative_path(&conn, relative_path)?;
        let path = self.canonical_document_path(&path)?;
        let source = if is_daily_note_relative_path(relative_path) {
            upsert_frontmatter_field(source, "last_updated", &iso_timestamp_now())?
        } else {
            source.to_string()
        };
        fs::write(&path, source).with_context(|| format!("write {}", path.display()))
    }

    fn document_path_by_relative_path(
        &self,
        conn: &Connection,
        relative_path: &str,
    ) -> Result<PathBuf> {
        conn.query_row(
            "select path from documents where relative_path = ?1",
            [relative_path],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(PathBuf::from)
        .with_context(|| format!("document not found: {relative_path}"))
    }

    fn canonical_document_path(&self, path: &Path) -> Result<PathBuf> {
        let path = path
            .canonicalize()
            .with_context(|| format!("resolve {}", path.display()))?;
        let root = self
            .root
            .canonicalize()
            .with_context(|| format!("resolve {}", self.root.display()))?;
        if !path.starts_with(&root) {
            anyhow::bail!("document path is outside vault: {}", path.display());
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            anyhow::bail!("document is not a markdown file: {}", path.display());
        }
        Ok(path)
    }

    fn canonical_vault_file_path(&self, path: &Path) -> Result<PathBuf> {
        let path = path
            .canonicalize()
            .with_context(|| format!("resolve {}", path.display()))?;
        let root = self
            .root
            .canonicalize()
            .with_context(|| format!("resolve {}", self.root.display()))?;
        if !path.starts_with(&root) {
            anyhow::bail!("file path is outside vault: {}", path.display());
        }
        Ok(path)
    }

    fn open_text_item(&self, file: &FileRow, path: &Path) -> VaultItemView {
        match fs::read_to_string(path) {
            Ok(source) => {
                let formatted = format_structured_source(&source, &file.extension);
                VaultItemView {
                    document_id: None,
                    slug: String::new(),
                    title: fallback_title(path),
                    filename: filename_for(path),
                    stem: stem_for(path),
                    path: path.to_path_buf(),
                    relative_path: file.relative_path.clone(),
                    kind: file.kind.clone(),
                    extension: file.extension.clone(),
                    size_bytes: file.size_bytes,
                    modified_at: file.modified_ns.and_then(nanos_to_secs),
                    html: None,
                    formatted: Some(formatted.unwrap_or_else(|error| error)),
                    source: Some(source),
                    media_data_url: None,
                    media_mime: None,
                    preview_message: None,
                    frontmatter: None,
                    frontmatter_error: None,
                    outgoing_links: Vec::new(),
                    backlinks: Vec::new(),
                    can_edit_source: false,
                    can_open_system: true,
                    error: None,
                }
            }
            Err(error) => {
                vault_item_error(file, path, format!("Could not read text file: {error}"))
            }
        }
    }

    fn sync(&self) -> Result<IndexSummary> {
        let mut conn = self.open_db()?;
        create_schema(&conn)?;
        let (index, fields, search_recreated) = open_or_create_search_index(&self.index_dir)?;
        let mut writer = index.writer(50_000_000).context("create tantivy writer")?;
        let discovered_files = discover_vault_files(&self.root)?;
        let discovered_relative_paths = discovered_files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let mut summary = IndexSummary {
            scanned: discovered_files.len(),
            ..IndexSummary::default()
        };
        let mut search_synced = Vec::new();
        let mut search_deleted = Vec::new();

        let tx = conn
            .transaction()
            .context("start sqlite index transaction")?;

        {
            let stale_hashes = stale_indexed_hashes(&tx, &discovered_relative_paths)?;

            for file in discovered_files {
                let file_state = file_index_state(&tx, &file.relative_path)?;
                let needs_document_update = file_state
                    .as_ref()
                    .map(|state| {
                        state.status != "indexed"
                            || state.content_hash != file.content_hash
                            || state.parser_version != PARSER_VERSION
                            || state.kind != file.kind
                            || state.extension != file.extension
                    })
                    .unwrap_or(true);
                let needs_search_update = search_recreated
                    || needs_document_update
                    || file_state
                        .as_ref()
                        .and_then(|state| state.search_hash.as_deref())
                        != Some(file.content_hash.as_str());

                if !needs_document_update && !needs_search_update {
                    summary.skipped += 1;
                    continue;
                }

                if file_state.is_none() && stale_hashes.contains(&file.content_hash) {
                    summary.renamed += 1;
                }

                if file.kind == "markdown" {
                    if needs_document_update {
                        let source = fs::read_to_string(&file.path)
                            .with_context(|| format!("read {}", file.path.display()))?;
                        let document = parse_markdown_source(&file.path, &self.root, &source)?;
                        let id = upsert_document(&tx, &document)?;
                        upsert_file_state(
                            &tx,
                            &file,
                            file_state
                                .as_ref()
                                .and_then(|state| state.search_hash.clone()),
                        )?;

                        if needs_search_update {
                            replace_search_document(&mut writer, &fields, id, &document)?;
                        }
                    } else if needs_search_update {
                        if let Some(document) =
                            document_for_search_by_relative_path(&tx, &file.relative_path)?
                        {
                            replace_search_row(&mut writer, &fields, &document)?;
                        }
                    }

                    if needs_search_update {
                        search_synced.push((file.relative_path.clone(), file.content_hash.clone()));
                    }
                } else {
                    remove_document_for_path(&tx, &file.relative_path)?;
                    upsert_file_state(&tx, &file, None)?;
                    if is_searchable_structured_extension(&file.extension) && needs_search_update {
                        replace_search_file(&mut writer, &fields, &file)?;
                        search_synced.push((file.relative_path.clone(), file.content_hash.clone()));
                    } else {
                        writer.delete_term(Term::from_field_text(
                            fields.doc_key,
                            &file.relative_path,
                        ));
                        search_deleted.push(file.relative_path.clone());
                    }
                }

                summary.updated += 1;
            }

            let mut deleted_paths = std::collections::BTreeSet::new();
            let indexed_paths = indexed_file_paths(&tx)?;
            for relative_path in indexed_paths {
                if discovered_relative_paths.contains(&relative_path) {
                    continue;
                }
                remove_document_for_path(&tx, &relative_path)?;
                tx.execute(
                    "update files set status = 'deleted', indexed_at = ?2 where relative_path = ?1",
                    params![relative_path, unix_timestamp()],
                )?;
                writer.delete_term(Term::from_field_text(fields.doc_key, &relative_path));
                search_deleted.push(relative_path.clone());
                deleted_paths.insert(relative_path);
                summary.deleted += 1;
            }

            for relative_path in document_paths(&tx)? {
                if discovered_relative_paths.contains(&relative_path) {
                    continue;
                }
                remove_document_for_path(&tx, &relative_path)?;
                writer.delete_term(Term::from_field_text(fields.doc_key, &relative_path));
                search_deleted.push(relative_path.clone());
                if deleted_paths.insert(relative_path) {
                    summary.deleted += 1;
                }
            }
        }

        tx.commit().context("commit sqlite index transaction")?;
        writer.commit().context("commit tantivy index")?;
        mark_search_synced(&conn, &search_synced, &search_deleted)?;
        Ok(summary)
    }

    fn open_db(&self) -> Result<Connection> {
        Connection::open(&self.db_path).with_context(|| format!("open {}", self.db_path.display()))
    }

    fn open_search_index(&self) -> Result<(Index, SearchFields)> {
        let schema = search_schema().0;
        let index = Index::open_in_dir(&self.index_dir).context("open tantivy index")?;
        let fields = fields_from_schema(&schema)?;
        Ok((index, fields))
    }
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        create table if not exists documents (
          id integer primary key autoincrement,
          slug text not null,
          title text not null,
          filename text not null,
          stem text not null,
          path text not null,
          relative_path text not null,
          body text not null,
          frontmatter_json text,
          frontmatter_error text
        );
        create table if not exists links (
          id integer primary key autoincrement,
          source_slug text not null,
          target_slug text not null,
          label text not null
        );
        create table if not exists files (
          relative_path text primary key,
          absolute_path text not null,
          kind text not null,
          extension text not null,
          size_bytes integer not null,
          modified_ns integer,
          content_hash text not null,
          search_hash text,
          indexed_at integer not null,
          parser_version text not null,
          status text not null
        );
        create index if not exists documents_slug on documents(slug);
        create index if not exists documents_relative_path on documents(relative_path);
        create index if not exists links_source on links(source_slug);
        create index if not exists links_target on links(target_slug);
        create index if not exists files_status on files(status);
        "#,
    )?;
    ensure_column(
        conn,
        "files",
        "kind",
        "alter table files add column kind text not null default 'markdown'",
    )?;
    ensure_column(
        conn,
        "files",
        "extension",
        "alter table files add column extension text not null default 'md'",
    )?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<()> {
    let mut statement = conn.prepare(&format!("pragma table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column {
            return Ok(());
        }
    }
    conn.execute(ddl, [])?;
    Ok(())
}

fn file_index_state(conn: &Connection, relative_path: &str) -> Result<Option<FileIndexState>> {
    conn.query_row(
        "select content_hash, search_hash, parser_version, status, kind, extension from files where relative_path = ?1",
        [relative_path],
        |row| {
            Ok(FileIndexState {
                content_hash: row.get(0)?,
                search_hash: row.get(1)?,
                parser_version: row.get(2)?,
                status: row.get(3)?,
                kind: row.get(4)?,
                extension: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn file_row_by_relative_path(conn: &Connection, relative_path: &str) -> Result<Option<FileRow>> {
    conn.query_row(
        "select relative_path, absolute_path, kind, extension, size_bytes, modified_ns from files where relative_path = ?1 and status = 'indexed'",
        [relative_path],
        |row| {
            Ok(FileRow {
                relative_path: row.get(0)?,
                absolute_path: PathBuf::from(row.get::<_, String>(1)?),
                kind: row.get(2)?,
                extension: row.get(3)?,
                size_bytes: row.get(4)?,
                modified_ns: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn append_file_search_hits(
    conn: &Connection,
    hits: &mut Vec<SearchHit>,
    query: &str,
    limit: usize,
) -> Result<()> {
    if hits.len() >= limit || query.is_empty() {
        return Ok(());
    }

    let existing_paths = hits
        .iter()
        .map(|hit| hit.relative_path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let pattern = format!("%{query}%");
    let mut statement = conn.prepare(
        r#"
        select d.id, d.slug, d.title, f.absolute_path, f.relative_path, f.kind, f.extension, f.size_bytes, f.modified_ns
        from files f
        left join documents d on d.relative_path = f.relative_path
        where f.status = 'indexed'
          and lower(f.relative_path) like ?1
        order by f.relative_path
        limit ?2
        "#,
    )?;
    let rows = statement.query_map(params![pattern, limit as i64], |row| {
        Ok((
            row.get::<_, Option<i64>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, Option<i64>>(8)?,
        ))
    })?;

    for row in rows {
        if hits.len() >= limit {
            break;
        }
        let (id, slug, title, path, relative_path, kind, extension, size_bytes, modified_ns) = row?;
        if existing_paths.contains(&relative_path) {
            continue;
        }
        let path = PathBuf::from(path);
        let filename = filename_for(&path);
        let stem = stem_for(&path);
        hits.push(SearchHit {
            id,
            slug: slug.unwrap_or_default(),
            title: title.unwrap_or_else(|| fallback_title(&path)),
            filename,
            stem,
            path,
            relative_path: relative_path.clone(),
            kind: kind.clone(),
            extension,
            size_bytes,
            snippet: format_file_snippet(&relative_path, &kind, size_bytes, modified_ns),
            score: 0.0,
        });
    }
    Ok(())
}

fn upsert_document(conn: &Connection, document: &IndexedDocument) -> Result<i64> {
    let existing = conn
        .query_row(
            "select id, slug from documents where relative_path = ?1",
            [&document.relative_path],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((id, old_slug)) = existing {
        conn.execute("delete from links where source_slug = ?1", [old_slug])?;
        conn.execute(
            "update documents set slug = ?1, title = ?2, filename = ?3, stem = ?4, path = ?5, body = ?6, frontmatter_json = ?7, frontmatter_error = ?8 where id = ?9",
            params![
                document.slug,
                document.title,
                document.filename,
                document.stem,
                document.path.to_string_lossy(),
                document.body,
                document
                    .frontmatter
                    .as_ref()
                    .map(serde_json::Value::to_string),
                document.frontmatter_error,
                id,
            ],
        )?;
        insert_links(conn, document)?;
        Ok(id)
    } else {
        conn.execute(
            "insert into documents (slug, title, filename, stem, path, relative_path, body, frontmatter_json, frontmatter_error) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                document.slug,
                document.title,
                document.filename,
                document.stem,
                document.path.to_string_lossy(),
                document.relative_path,
                document.body,
                document
                    .frontmatter
                    .as_ref()
                    .map(serde_json::Value::to_string),
                document.frontmatter_error,
            ],
        )?;
        let id = conn.last_insert_rowid();
        insert_links(conn, document)?;
        Ok(id)
    }
}

fn insert_links(conn: &Connection, document: &IndexedDocument) -> Result<()> {
    let mut insert_link =
        conn.prepare("insert into links (source_slug, target_slug, label) values (?1, ?2, ?3)")?;
    for link in &document.links {
        insert_link.execute(params![document.slug, link.target, link.label])?;
    }
    Ok(())
}

fn upsert_file_state(
    conn: &Connection,
    file: &DiscoveredFile,
    previous_search_hash: Option<String>,
) -> Result<()> {
    conn.execute(
        r#"
        insert into files (
          relative_path, absolute_path, kind, extension, size_bytes, modified_ns, content_hash,
          search_hash, indexed_at, parser_version, status
        ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'indexed')
        on conflict(relative_path) do update set
          absolute_path = excluded.absolute_path,
          kind = excluded.kind,
          extension = excluded.extension,
          size_bytes = excluded.size_bytes,
          modified_ns = excluded.modified_ns,
          content_hash = excluded.content_hash,
          search_hash = excluded.search_hash,
          indexed_at = excluded.indexed_at,
          parser_version = excluded.parser_version,
          status = 'indexed'
        "#,
        params![
            file.relative_path,
            file.path.to_string_lossy(),
            file.kind,
            file.extension,
            file.size_bytes,
            file.modified_ns,
            file.content_hash,
            previous_search_hash,
            unix_timestamp(),
            PARSER_VERSION,
        ],
    )?;
    Ok(())
}

fn indexed_file_paths(conn: &Connection) -> Result<Vec<String>> {
    let mut statement = conn.prepare("select relative_path from files where status = 'indexed'")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn stale_indexed_hashes(
    conn: &Connection,
    discovered_relative_paths: &std::collections::BTreeSet<String>,
) -> Result<std::collections::BTreeSet<String>> {
    let mut statement =
        conn.prepare("select relative_path, content_hash from files where status = 'indexed'")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut hashes = std::collections::BTreeSet::new();
    for row in rows {
        let (relative_path, content_hash) = row?;
        if !discovered_relative_paths.contains(&relative_path) {
            hashes.insert(content_hash);
        }
    }
    Ok(hashes)
}

fn document_paths(conn: &Connection) -> Result<Vec<String>> {
    let mut statement = conn.prepare("select relative_path from documents")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn document_slug_by_relative_path(
    conn: &Connection,
    relative_path: &str,
) -> Result<Option<String>> {
    conn.query_row(
        "select slug from documents where relative_path = ?1",
        [relative_path],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn remove_document_for_path(conn: &Connection, relative_path: &str) -> Result<()> {
    if let Some(slug) = document_slug_by_relative_path(conn, relative_path)? {
        conn.execute("delete from links where source_slug = ?1", [slug])?;
    }
    conn.execute(
        "delete from documents where relative_path = ?1",
        [relative_path],
    )?;
    Ok(())
}

fn document_for_search_by_relative_path(
    conn: &Connection,
    relative_path: &str,
) -> Result<Option<SearchDocumentRow>> {
    conn.query_row(
        "select id, slug, title, filename, relative_path, body from documents where relative_path = ?1",
        [relative_path],
        |row| {
            Ok(SearchDocumentRow {
                id: row.get(0)?,
                slug: row.get(1)?,
                title: row.get(2)?,
                filename: row.get(3)?,
                relative_path: row.get(4)?,
                body: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn mark_search_synced(
    conn: &Connection,
    synced: &[(String, String)],
    deleted: &[String],
) -> Result<()> {
    for (relative_path, content_hash) in synced {
        conn.execute(
            "update files set search_hash = ?2 where relative_path = ?1 and status = 'indexed'",
            params![relative_path, content_hash],
        )?;
    }
    for relative_path in deleted {
        conn.execute(
            "update files set search_hash = null where relative_path = ?1 and status = 'deleted'",
            [relative_path],
        )?;
    }
    Ok(())
}

fn reset_runtime_state_dir(state_dir: &Path) -> Result<()> {
    let db_path = state_dir.join("mega-vault-viewer.sqlite");
    for path in [
        db_path.clone(),
        sqlite_sidecar_path(&db_path, "-wal"),
        sqlite_sidecar_path(&db_path, "-shm"),
        sqlite_sidecar_path(&db_path, "-journal"),
    ] {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("remove runtime cache file {}", path.display()))?;
        }
    }

    for dir_name in ["tantivy", "thumbnails", "render-cache"] {
        let path = state_dir.join(dir_name);
        if path.exists() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("remove runtime cache directory {}", path.display()))?;
        }
    }

    Ok(())
}

fn sqlite_sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut path = db_path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

fn starter_contract_path(root: &Path) -> PathBuf {
    root.join("docs/starter-contract.json")
}

fn load_starter_contract(root: &Path) -> Option<StarterContract> {
    let path = starter_contract_path(root);
    let source = fs::read_to_string(path).ok()?;
    let contract = serde_json::from_str::<StarterContract>(&source).ok()?;
    let looks_like_minimal_starter = contract
        .canonical
        .iter()
        .any(|entry| entry == "records/*.jsonl")
        || contract
            .name
            .as_deref()
            .unwrap_or_default()
            .contains("Minimal AI Vault Starter");
    looks_like_minimal_starter.then_some(contract)
}

fn can_edit_markdown_source(root: &Path, relative_path: &str) -> bool {
    let Some(contract) = load_starter_contract(root) else {
        return true;
    };
    !contract
        .human_owned
        .iter()
        .any(|pattern| starter_glob_matches(pattern, relative_path))
}

fn starter_glob_matches(pattern: &str, relative_path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return relative_path == prefix || relative_path.starts_with(&format!("{prefix}/"));
    }
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        return relative_path.starts_with(prefix) && relative_path.ends_with(suffix);
    }
    pattern == relative_path
}

fn starter_vault_summary(
    root: &Path,
    files: &[FileBrowserItem],
) -> Result<Option<StarterVaultSummary>> {
    let Some(contract) = load_starter_contract(root) else {
        return Ok(None);
    };
    let record_collections = starter_record_collections(root)?;
    let total_records = record_collections
        .iter()
        .map(|collection| collection.count)
        .sum();
    let human_note_count = files
        .iter()
        .filter(|file| {
            contract
                .human_owned
                .iter()
                .any(|pattern| starter_glob_matches(pattern, &file.relative_path))
        })
        .count();
    let generated_view_count = files
        .iter()
        .filter(|file| {
            contract
                .generated
                .iter()
                .any(|pattern| starter_glob_matches(pattern, &file.relative_path))
        })
        .count();

    Ok(Some(StarterVaultSummary {
        name: contract
            .name
            .unwrap_or_else(|| "Minimal AI Vault Starter".to_string()),
        promise: contract.promise.unwrap_or_default(),
        human_owned: contract.human_owned,
        canonical: contract.canonical,
        generated: contract.generated,
        record_collections,
        total_records,
        human_note_count,
        generated_view_count,
    }))
}

fn starter_record_collections(root: &Path) -> Result<Vec<StarterRecordCollection>> {
    let records_dir = root.join("records");
    if !records_dir.exists() {
        return Ok(Vec::new());
    }
    let mut collections = Vec::new();
    for entry in fs::read_dir(records_dir).context("read starter records directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
            continue;
        }
        let file_name = filename_for(&path);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read starter record collection {}", path.display()))?;
        let mut count = 0;
        let mut record_type = None;
        let mut status = "ok".to_string();
        for (index, line) in source.split('\n').enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            count += 1;
            match serde_json::from_str::<serde_json::Value>(trimmed) {
                Ok(value) => {
                    if record_type.is_none() {
                        record_type = value
                            .get("record_type")
                            .or_else(|| value.get("type"))
                            .and_then(|value| value.as_str())
                            .map(str::to_string);
                    }
                }
                Err(error) => {
                    status = format!("parse issue on line {}: {error}", index + 1);
                    break;
                }
            }
        }
        let schema_name = starter_schema_name(file_name.trim_end_matches(".jsonl"));
        let schema = root
            .join("schema")
            .join(format!("{schema_name}.schema.json"));
        collections.push(StarterRecordCollection {
            file: format!("records/{file_name}"),
            schema: schema
                .exists()
                .then(|| format!("schema/{schema_name}.schema.json")),
            count,
            record_type,
            status,
        });
    }
    collections.sort_by(|left, right| left.file.cmp(&right.file));
    Ok(collections)
}

fn starter_schema_name(collection_name: &str) -> &str {
    match collection_name {
        "attachments" => "attachment",
        "claims" => "claim",
        "decisions" => "decision",
        "entities" => "entity",
        "projects" => "project",
        "relations" => "relation",
        "sources" => "source",
        "tasks" => "task",
        other => other.trim_end_matches('s'),
    }
}

fn discover_vault_files(root: &Path) -> Result<Vec<DiscoveredFile>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_descend_vault_entry(root, entry.path()))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let metadata =
            fs::metadata(path).with_context(|| format!("read metadata {}", path.display()))?;
        let extension = normalized_extension(path);
        files.push(DiscoveredFile {
            path: path.to_path_buf(),
            relative_path: relative_path_for(root, path),
            kind: kind_for_extension(&extension).to_string(),
            extension,
            size_bytes: metadata.len() as i64,
            modified_ns: metadata.modified().ok().and_then(system_time_nanos),
            content_hash: content_hash_for_discovery(path, &metadata)?,
        });
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn should_descend_vault_entry(root: &Path, path: &Path) -> bool {
    if path == root {
        return true;
    }
    let Ok(relative_path) = path.strip_prefix(root) else {
        return true;
    };
    let Some(first_component) = relative_path.components().next() else {
        return true;
    };
    let first_component = first_component.as_os_str().to_string_lossy();
    !matches!(
        first_component.as_ref(),
        ".git"
            | ".obsidian"
            | ".trash"
            | "blobs"
            | "indexes"
            | "node_modules"
            | "target"
            | "mega-vault-viewer.sqlite"
            | "tantivy"
    )
}

fn content_hash_for_discovery(path: &Path, metadata: &fs::Metadata) -> Result<String> {
    if is_markdown_path(path) {
        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        return Ok(content_hash_bytes(&bytes));
    }

    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(system_time_nanos)
        .unwrap_or(0);
    Ok(content_hash_bytes(
        format!(
            "mvv-metadata-v1:{}:{}:{}",
            normalized_extension(path),
            metadata.len(),
            modified_ns
        )
        .as_bytes(),
    ))
}

fn is_markdown_path(path: &Path) -> bool {
    matches!(normalized_extension(path).as_str(), "md" | "markdown")
}

fn is_searchable_structured_extension(extension: &str) -> bool {
    matches!(extension, "json" | "jsonl" | "yaml" | "yml")
}

fn searchable_structured_source(path: &Path, extension: &str) -> Result<String> {
    let source = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if extension == "json" {
        return Ok(format_structured_source(&source, extension).unwrap_or(source));
    }
    Ok(source)
}

fn normalized_extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

fn kind_for_extension(extension: &str) -> &'static str {
    match extension {
        "md" | "markdown" => "markdown",
        "yaml" | "yml" => "yaml",
        "json" | "jsonl" => "json",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "svg" => "image",
        "pdf" => "pdf",
        "csv" | "tsv" | "xlsx" | "xls" => "data",
        _ => "file",
    }
}

fn parse_markdown_source(path: &Path, root: &Path, source: &str) -> Result<IndexedDocument> {
    let (frontmatter, frontmatter_error, body) = split_frontmatter(source);
    let filename = filename_for(path);
    let stem = stem_for(path);
    let absolute_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let relative_path = relative_path_for(root, path);
    let title = frontmatter
        .as_ref()
        .and_then(|value| value.get("title"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| first_heading(&body))
        .unwrap_or_else(|| fallback_title(path));
    let slug = frontmatter
        .as_ref()
        .and_then(|value| value.get("slug"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("untitled")
                .to_string()
        });
    let links = extract_wikilinks(&body);

    Ok(IndexedDocument {
        slug,
        title,
        filename,
        stem,
        path: absolute_path,
        relative_path,
        body,
        frontmatter,
        frontmatter_error,
        links,
    })
}

fn content_hash_bytes(source: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source);
    format!("{:x}", hasher.finalize())
}

fn filename_for(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("untitled.md")
        .to_string()
}

fn stem_for(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("untitled")
        .to_string()
}

fn relative_path_for(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn split_frontmatter(source: &str) -> (Option<serde_json::Value>, Option<String>, String) {
    if !source.starts_with("---\n") {
        return (None, None, source.to_string());
    }
    let Some(rest) = source.strip_prefix("---\n") else {
        return (None, None, source.to_string());
    };
    let Some(end) = rest.find("\n---\n") else {
        return (
            None,
            Some("frontmatter closing delimiter not found".to_string()),
            source.to_string(),
        );
    };
    let yaml = &rest[..end];
    let body = rest[end + "\n---\n".len()..].to_string();
    match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => (Some(value), None, body),
            Err(error) => (
                None,
                Some(format!("frontmatter conversion failed: {error}")),
                body,
            ),
        },
        Err(error) => (
            parse_frontmatter_fallback(yaml),
            Some(format!("frontmatter parse failed: {error}")),
            body,
        ),
    }
}

fn parse_frontmatter_fallback(yaml: &str) -> Option<serde_json::Value> {
    let mut object = serde_json::Map::new();
    for line in yaml.lines() {
        if line.starts_with(' ') || line.starts_with('\t') || line.trim_start().starts_with('-') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        if value.starts_with('[') && value.ends_with(']') {
            let values = value
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|item| item.trim().trim_matches('"').trim_matches('\''))
                .filter(|item| !item.is_empty())
                .map(|item| serde_json::Value::String(item.to_string()))
                .collect::<Vec<_>>();
            object.insert(key.to_string(), serde_json::Value::Array(values));
            continue;
        }
        if value.starts_with('[') || value.starts_with('{') {
            continue;
        }
        object.insert(
            key.to_string(),
            serde_json::Value::String(value.trim_matches('"').trim_matches('\'').to_string()),
        );
    }
    if object.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(object))
    }
}

fn first_heading(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
}

fn fallback_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled")
        .to_string()
}

fn extract_wikilinks(body: &str) -> Vec<WikiLink> {
    let re = Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").expect("valid wikilink regex");
    re.captures_iter(body)
        .filter(|captures| {
            let start = captures.get(0).map(|matched| matched.start()).unwrap_or(0);
            start == 0 || !body[..start].ends_with('!')
        })
        .map(|captures| {
            let target = captures.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            let label = captures
                .get(2)
                .map(|m| m.as_str().trim())
                .filter(|value| !value.is_empty())
                .unwrap_or(target);
            WikiLink {
                target: target.to_string(),
                label: label.to_string(),
            }
        })
        .filter(|link| !link.target.is_empty())
        .collect()
}

fn render_markdown(body: &str, vault_root: &Path, document_path: &Path) -> String {
    let markdown = replace_obsidian_callouts(body);
    let markdown = replace_inline_tags(&markdown);
    let markdown = replace_obsidian_image_embeds(&markdown, vault_root, document_path);
    let markdown = replace_markdown_images(&markdown, vault_root, document_path);
    let re = Regex::new(r"\[\[([^\]|!]+)(?:\|([^\]]+))?\]\]").expect("valid wikilink regex");
    let markdown = re.replace_all(&markdown, |captures: &regex::Captures<'_>| {
        let target = captures.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let label = captures
            .get(2)
            .map(|m| m.as_str().trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(target);
        format!(
            "[{label}](mvv://open/{})",
            percent_encode_link_target(target)
        )
    });
    catch_unwind_silent(|| {
        let parser = Parser::new_ext(&markdown, Options::all());
        let mut rendered = String::new();
        html::push_html(&mut rendered, parser);
        rendered
    })
    .unwrap_or_else(|_| render_plaintext_fallback(body))
}

fn replace_obsidian_callouts(body: &str) -> String {
    let marker = Regex::new(r"^>\s*\[!([A-Za-z0-9_-]+)\]\s*(.*)$").expect("valid callout regex");
    let mut output = Vec::new();
    let lines = body.lines().collect::<Vec<_>>();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let Some(captures) = marker.captures(line) else {
            output.push(line.to_string());
            index += 1;
            continue;
        };

        let kind = captures
            .get(1)
            .map(|matched| sanitize_callout_kind(matched.as_str()))
            .unwrap_or_else(|| "note".to_string());
        let title = captures
            .get(2)
            .map(|matched| matched.as_str().trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(kind.as_str());
        index += 1;

        let mut body_lines = Vec::new();
        while index < lines.len() {
            let next = lines[index];
            if !next.trim_start().starts_with('>') {
                break;
            }
            let content = next
                .trim_start()
                .strip_prefix('>')
                .unwrap_or(next)
                .strip_prefix(' ')
                .unwrap_or_else(|| next.trim_start().strip_prefix('>').unwrap_or(next));
            body_lines.push(content.to_string());
            index += 1;
        }

        output.push(render_callout_html(&kind, title, &body_lines));
    }

    output.join("\n")
}

fn render_callout_html(kind: &str, title: &str, body_lines: &[String]) -> String {
    let mut html = format!(
        r#"<aside class="callout callout-{}"><p class="callout-title">{}</p>"#,
        kind,
        escape_html(title)
    );
    for paragraph in body_lines
        .split(|line| line.trim().is_empty())
        .filter(|paragraph| !paragraph.is_empty())
    {
        html.push_str("<p>");
        html.push_str(&escape_html(&paragraph.join(" ")));
        html.push_str("</p>");
    }
    html.push_str("</aside>");
    html
}

fn sanitize_callout_kind(value: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    if sanitized.is_empty() {
        "note".to_string()
    } else {
        sanitized
    }
}

fn replace_inline_tags(body: &str) -> String {
    let tag = Regex::new(r"(^|[\s(])#([A-Za-z][A-Za-z0-9_/-]*)\b").expect("valid tag regex");
    let mut in_fence = false;
    body.lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                return line.to_string();
            }
            if in_fence || is_markdown_heading(line) {
                return line.to_string();
            }
            tag.replace_all(line, |captures: &regex::Captures<'_>| {
                let prefix = captures
                    .get(1)
                    .map(|matched| matched.as_str())
                    .unwrap_or("");
                let value = captures
                    .get(2)
                    .map(|matched| matched.as_str())
                    .unwrap_or("");
                format!(
                    r#"{prefix}<span class="vault-tag">#{}</span>"#,
                    escape_html(value)
                )
            })
            .into_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_markdown_heading(line: &str) -> bool {
    let trimmed = line.trim_start();
    let marker_width = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (1..=6).contains(&marker_width) && trimmed.chars().nth(marker_width) == Some(' ')
}

fn percent_encode_link_target(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn replace_obsidian_image_embeds(body: &str, vault_root: &Path, document_path: &Path) -> String {
    let re = Regex::new(r"!\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").expect("valid embed regex");
    re.replace_all(body, |captures: &regex::Captures<'_>| {
        let target = captures.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let alt = captures
            .get(2)
            .map(|m| m.as_str().trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(target);
        render_media_html(target, alt, vault_root, document_path)
    })
    .into_owned()
}

fn replace_markdown_images(body: &str, vault_root: &Path, document_path: &Path) -> String {
    let re = Regex::new(r"!\[([^\]]*)\]\(([^)]+)\)").expect("valid markdown image regex");
    re.replace_all(body, |captures: &regex::Captures<'_>| {
        let alt = captures.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let target = captures.get(2).map(|m| m.as_str()).unwrap_or("").trim();
        if is_external_media_target(target) {
            return captures
                .get(0)
                .map(|matched| matched.as_str().to_string())
                .unwrap_or_default();
        }
        render_media_html(target, alt, vault_root, document_path)
    })
    .into_owned()
}

fn render_media_html(target: &str, alt: &str, vault_root: &Path, document_path: &Path) -> String {
    if let Some(hash) = parse_cas_sha256_target(target) {
        return render_cas_media_html(&hash, alt, vault_root);
    }

    let Some(path) = resolve_media_path(target, vault_root, document_path) else {
        return format!(
            r#"<span class="missing-media">Missing media: {}</span>"#,
            escape_html(target)
        );
    };
    let Some(mime) = mime_for_path(&path) else {
        return format!(
            r#"<span class="missing-media">Unsupported media: {}</span>"#,
            escape_html(target)
        );
    };
    match fs::read(&path) {
        Ok(bytes) => format!(
            r#"<img class="vault-image" src="data:{};base64,{}" alt="{}" loading="lazy" />"#,
            mime,
            general_purpose::STANDARD.encode(bytes),
            escape_html(alt)
        ),
        Err(_) => format!(
            r#"<span class="missing-media">Missing media: {}</span>"#,
            escape_html(target)
        ),
    }
}

fn render_cas_media_html(hash: &str, alt: &str, vault_root: &Path) -> String {
    let path = vault_root
        .join("blobs")
        .join("sha256")
        .join(&hash[..2])
        .join(hash);
    if !path.is_file() {
        return format!(
            r#"<span class="missing-media">Missing CAS blob: {}</span>"#,
            escape_html(hash)
        );
    }
    match fs::read(&path) {
        Ok(bytes) => {
            let Some(mime) = mime_for_bytes(&bytes) else {
                return format!(
                    r#"<span class="missing-media">Unsupported CAS media: {}</span>"#,
                    escape_html(hash)
                );
            };
            format!(
                r#"<img class="vault-image vault-image-cas" data-cas-sha256="{}" src="data:{};base64,{}" alt="{}" loading="lazy" />"#,
                escape_html(hash),
                mime,
                general_purpose::STANDARD.encode(bytes),
                escape_html(alt)
            )
        }
        Err(_) => format!(
            r#"<span class="missing-media">Missing CAS blob: {}</span>"#,
            escape_html(hash)
        ),
    }
}

fn parse_cas_sha256_target(target: &str) -> Option<String> {
    let cleaned = target.trim();
    let hash = cleaned
        .strip_prefix("cas-sha256-")
        .or_else(|| cleaned.strip_prefix("cas:sha256:"))
        .or_else(|| cleaned.strip_prefix("cassha256:"))
        .or_else(|| cleaned.strip_prefix("cassha256"))
        .or_else(|| cleaned.strip_prefix("blob://sha256/"))
        .or_else(|| cleaned.strip_prefix("file.sha256."))?;
    let hash = hash.trim();
    if hash.len() == 64 && hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(hash.to_ascii_lowercase())
    } else {
        None
    }
}

fn resolve_media_path(target: &str, vault_root: &Path, document_path: &Path) -> Option<PathBuf> {
    if is_external_media_target(target) {
        return None;
    }

    let cleaned = clean_media_target(target);
    let target_path = PathBuf::from(&cleaned);
    let mut candidates = Vec::new();
    if target_path.is_absolute() {
        candidates.push(target_path);
    } else {
        if let Some(parent) = document_path.parent() {
            candidates.push(parent.join(&cleaned));
        }
        candidates.push(vault_root.join(&cleaned));
    }

    for candidate in candidates {
        if let Some(path) = canonical_media_path(&candidate, vault_root) {
            return Some(path);
        }
    }

    let filename = Path::new(&cleaned).file_name()?.to_str()?;
    find_media_by_filename(vault_root, filename)
}

fn clean_media_target(target: &str) -> String {
    target
        .trim_matches('<')
        .trim_matches('>')
        .split('#')
        .next()
        .unwrap_or(target)
        .split('?')
        .next()
        .unwrap_or(target)
        .replace("%20", " ")
}

fn is_external_media_target(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
        || lower.starts_with("file://")
}

fn canonical_media_path(candidate: &Path, vault_root: &Path) -> Option<PathBuf> {
    let path = candidate.canonicalize().ok()?;
    let root = vault_root.canonicalize().ok()?;
    if path.starts_with(root) && path.is_file() && mime_for_path(&path).is_some() {
        Some(path)
    } else {
        None
    }
}

fn find_media_by_filename(vault_root: &Path, filename: &str) -> Option<PathBuf> {
    for folder in ["30_media", "20_files", "attachments", "assets"] {
        let root = vault_root.join(folder);
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if entry.file_type().is_file()
                && path.file_name().and_then(|name| name.to_str()) == Some(filename)
            {
                if let Some(path) = canonical_media_path(path, vault_root) {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn mime_for_path(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("svg") => Some("image/svg+xml"),
        Some("bmp") => Some("image/bmp"),
        Some("avif") => Some("image/avif"),
        Some("heic") => Some("image/heic"),
        _ => None,
    }
}

fn mime_for_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        Some("image/svg+xml")
    } else {
        None
    }
}

fn catch_unwind_silent<T>(f: impl FnOnce() -> T) -> std::thread::Result<T> {
    static PANIC_HOOK_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = PANIC_HOOK_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().expect("panic hook lock poisoned");
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    result
}

fn render_plaintext_fallback(body: &str) -> String {
    format!("<pre>{}</pre>", escape_html(body))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn create_search_index(index_dir: &Path) -> Result<(Index, SearchFields)> {
    let (schema, fields) = search_schema();
    let index = Index::create_in_dir(index_dir, schema).context("create tantivy index")?;
    Ok((index, fields))
}

fn open_or_create_search_index(index_dir: &Path) -> Result<(Index, SearchFields, bool)> {
    fs::create_dir_all(index_dir).context("create tantivy index directory")?;
    match Index::open_in_dir(index_dir) {
        Ok(index) => {
            let schema = index.schema();
            match fields_from_schema(&schema) {
                Ok(fields) => Ok((index, fields, false)),
                Err(_) => recreate_search_index(index_dir),
            }
        }
        Err(_) => recreate_search_index(index_dir),
    }
}

fn recreate_search_index(index_dir: &Path) -> Result<(Index, SearchFields, bool)> {
    if index_dir.exists() {
        fs::remove_dir_all(index_dir).context("clear incompatible tantivy index")?;
    }
    fs::create_dir_all(index_dir).context("create tantivy index directory")?;
    let (index, fields) = create_search_index(index_dir)?;
    Ok((index, fields, true))
}

fn search_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();
    let id = builder.add_u64_field("id", INDEXED | STORED | FAST);
    let doc_key = builder.add_text_field("doc_key", STRING | STORED);
    let slug = builder.add_text_field("slug", STRING | STORED);
    let title = builder.add_text_field("title", TEXT | STORED);
    let filename = builder.add_text_field("filename", TEXT | STORED);
    let relative_path = builder.add_text_field("relative_path", TEXT | STORED);
    let body = builder.add_text_field("body", TEXT | STORED);
    let schema = builder.build();
    (
        schema,
        SearchFields {
            id,
            doc_key,
            slug,
            title,
            filename,
            relative_path,
            body,
        },
    )
}

fn fields_from_schema(schema: &Schema) -> Result<SearchFields> {
    Ok(SearchFields {
        id: schema.get_field("id")?,
        doc_key: schema.get_field("doc_key")?,
        slug: schema.get_field("slug")?,
        title: schema.get_field("title")?,
        filename: schema.get_field("filename")?,
        relative_path: schema.get_field("relative_path")?,
        body: schema.get_field("body")?,
    })
}

fn replace_search_document(
    writer: &mut IndexWriter,
    fields: &SearchFields,
    id: i64,
    document: &IndexedDocument,
) -> Result<()> {
    writer.delete_term(Term::from_field_text(
        fields.doc_key,
        &document.relative_path,
    ));
    writer.add_document(doc!(
        fields.id => id as u64,
        fields.doc_key => document.relative_path.as_str(),
        fields.slug => document.slug.as_str(),
        fields.title => document.title.as_str(),
        fields.filename => document.filename.as_str(),
        fields.relative_path => document.relative_path.as_str(),
        fields.body => document.body.as_str(),
    ))?;
    Ok(())
}

fn replace_search_row(
    writer: &mut IndexWriter,
    fields: &SearchFields,
    document: &SearchDocumentRow,
) -> Result<()> {
    writer.delete_term(Term::from_field_text(
        fields.doc_key,
        &document.relative_path,
    ));
    writer.add_document(doc!(
        fields.id => document.id as u64,
        fields.doc_key => document.relative_path.as_str(),
        fields.slug => document.slug.as_str(),
        fields.title => document.title.as_str(),
        fields.filename => document.filename.as_str(),
        fields.relative_path => document.relative_path.as_str(),
        fields.body => document.body.as_str(),
    ))?;
    Ok(())
}

fn replace_search_file(
    writer: &mut IndexWriter,
    fields: &SearchFields,
    file: &DiscoveredFile,
) -> Result<()> {
    writer.delete_term(Term::from_field_text(fields.doc_key, &file.relative_path));
    let body = searchable_structured_source(&file.path, &file.extension)?;
    writer.add_document(doc!(
        fields.id => file_search_id(&file.relative_path),
        fields.doc_key => file.relative_path.as_str(),
        fields.slug => file.relative_path.as_str(),
        fields.title => fallback_title(&file.path).as_str(),
        fields.filename => filename_for(&file.path).as_str(),
        fields.relative_path => file.relative_path.as_str(),
        fields.body => body.as_str(),
    ))?;
    Ok(())
}

fn file_search_id(relative_path: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(b"mvv-file-search-v1");
    hasher.update(relative_path.as_bytes());
    let digest = hasher.finalize();
    u64::from_be_bytes(digest[..8].try_into().expect("sha256 digest has 8 bytes"))
}

fn first_u64(document: &TantivyDocument, field: Field) -> Option<u64> {
    document
        .get_first(field)
        .and_then(|value| match OwnedValue::from(value) {
            OwnedValue::U64(value) => Some(value),
            _ => None,
        })
}

fn first_text(document: &TantivyDocument, field: Field) -> Option<String> {
    document
        .get_first(field)
        .and_then(|value| match OwnedValue::from(value) {
            OwnedValue::Str(value) => Some(value),
            _ => None,
        })
}

fn document_summary(conn: &Connection, id: i64) -> Result<Option<DocumentSummary>> {
    conn.query_row(
        "select id, slug, title, filename, stem, path, relative_path, body from documents where id = ?1",
        [id],
        |row| {
            Ok(DocumentSummary {
                id: row.get(0)?,
                slug: row.get(1)?,
                title: row.get(2)?,
                filename: row.get(3)?,
                stem: row.get(4)?,
                path: PathBuf::from(row.get::<_, String>(5)?),
                relative_path: row.get(6)?,
                body: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn row_to_document_view(
    row: &rusqlite::Row<'_>,
    vault_root: &Path,
) -> rusqlite::Result<DocumentView> {
    let body: String = row.get(7)?;
    let frontmatter_json: Option<String> = row.get(8)?;
    let frontmatter = frontmatter_json.and_then(|value| serde_json::from_str(&value).ok());
    let path = PathBuf::from(row.get::<_, String>(5)?);
    Ok(DocumentView {
        id: row.get(0)?,
        slug: row.get(1)?,
        title: row.get(2)?,
        filename: row.get(3)?,
        stem: row.get(4)?,
        path: path.clone(),
        relative_path: row.get(6)?,
        html: render_markdown(&body, vault_root, &path),
        frontmatter,
        frontmatter_error: row.get(9)?,
        outgoing_links: Vec::new(),
        backlinks: Vec::new(),
    })
}

fn vault_item_from_document(
    document: DocumentView,
    file: &FileRow,
    can_edit_source: bool,
) -> VaultItemView {
    VaultItemView {
        document_id: Some(document.id),
        slug: document.slug,
        title: document.title,
        filename: document.filename,
        stem: document.stem,
        path: document.path,
        relative_path: document.relative_path,
        kind: file.kind.clone(),
        extension: file.extension.clone(),
        size_bytes: file.size_bytes,
        modified_at: file.modified_ns.and_then(nanos_to_secs),
        html: Some(document.html),
        formatted: None,
        source: None,
        media_data_url: None,
        media_mime: None,
        preview_message: None,
        frontmatter: document.frontmatter,
        frontmatter_error: document.frontmatter_error,
        outgoing_links: document.outgoing_links,
        backlinks: document.backlinks,
        can_edit_source,
        can_open_system: true,
        error: None,
    }
}

fn vault_item_preview(file: &FileRow, path: &Path, message: &str) -> VaultItemView {
    VaultItemView {
        document_id: None,
        slug: String::new(),
        title: fallback_title(path),
        filename: filename_for(path),
        stem: stem_for(path),
        path: path.to_path_buf(),
        relative_path: file.relative_path.clone(),
        kind: file.kind.clone(),
        extension: file.extension.clone(),
        size_bytes: file.size_bytes,
        modified_at: file.modified_ns.and_then(nanos_to_secs),
        html: None,
        formatted: None,
        source: None,
        media_data_url: None,
        media_mime: None,
        preview_message: Some(message.to_string()),
        frontmatter: None,
        frontmatter_error: None,
        outgoing_links: Vec::new(),
        backlinks: Vec::new(),
        can_edit_source: false,
        can_open_system: true,
        error: None,
    }
}

fn vault_item_error(file: &FileRow, path: &Path, message: String) -> VaultItemView {
    let mut item = vault_item_preview(file, path, "This file could not be previewed.");
    item.error = Some(message);
    item
}

fn open_image_item(file: &FileRow, path: &Path) -> VaultItemView {
    let Some(mime) = mime_for_path(path) else {
        return vault_item_error(file, path, "Unsupported image type".to_string());
    };
    match fs::read(path) {
        Ok(bytes) => {
            let mut item = vault_item_preview(file, path, "Image preview");
            item.media_mime = Some(mime.to_string());
            item.media_data_url = Some(format!(
                "data:{};base64,{}",
                mime,
                general_purpose::STANDARD.encode(bytes)
            ));
            item.preview_message = None;
            item
        }
        Err(error) => vault_item_error(file, path, format!("Could not read image: {error}")),
    }
}

fn open_generic_item(file: &FileRow, path: &Path) -> VaultItemView {
    if file.size_bytes <= 512_000 {
        if let Ok(source) = fs::read_to_string(path) {
            let mut item = vault_item_preview(file, path, "Text preview");
            item.source = Some(source.clone());
            item.formatted = Some(source);
            item.preview_message = None;
            return item;
        }
    }
    vault_item_preview(
        file,
        path,
        "No inline preview is available for this file type.",
    )
}

fn format_structured_source(source: &str, extension: &str) -> std::result::Result<String, String> {
    match extension {
        "json" => serde_json::from_str::<serde_json::Value>(source)
            .and_then(|value| serde_json::to_string_pretty(&value))
            .map_err(|error| format!("JSON parse issue: {error}\n\n{source}")),
        "jsonl" => format_jsonl_source(source),
        "yaml" | "yml" => serde_yaml::from_str::<serde_yaml::Value>(source)
            .map_err(|error| format!("YAML parse issue: {error}\n\n{source}"))
            .and_then(|value| {
                serde_json::to_string_pretty(
                    &serde_json::to_value(value)
                        .map_err(|error| format!("YAML conversion issue: {error}"))?,
                )
                .map_err(|error| format!("YAML formatting issue: {error}\n\n{source}"))
            }),
        _ => Ok(source.to_string()),
    }
}

fn format_jsonl_source(source: &str) -> std::result::Result<String, String> {
    let mut values = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value = serde_json::from_str::<serde_json::Value>(trimmed).map_err(|error| {
            format!(
                "JSONL parse issue on line {}: {error}\n\n{source}",
                index + 1
            )
        })?;
        values.push(value);
    }
    serde_json::to_string_pretty(&values)
        .map_err(|error| format!("JSONL formatting issue: {error}\n\n{source}"))
}

fn collect_strings(conn: &Connection, sql: &str, value: &str) -> Result<Vec<String>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([value], |row| row.get::<_, String>(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn file_browser_item(row: BrowserFileRow) -> FileBrowserItem {
    let metadata = fs::metadata(&row.path).ok();
    let filename = filename_for(&row.path);
    let modified_at = row.modified_ns.and_then(nanos_to_secs).or_else(|| {
        metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(system_time_seconds)
    });
    let created_at = metadata
        .as_ref()
        .and_then(|metadata| metadata.created().ok())
        .and_then(system_time_seconds)
        .or_else(|| timestamp_from_filename(&filename));

    FileBrowserItem {
        id: row.id,
        document_id: row.id,
        slug: row.slug,
        title: row.title.unwrap_or_else(|| fallback_title(&row.path)),
        filename,
        relative_path: row.relative_path,
        kind: row.kind,
        extension: row.extension,
        size_bytes: row.size_bytes,
        modified_at,
        created_at,
    }
}

fn folder_entries(files: &[FileBrowserItem]) -> Vec<FolderEntry> {
    use std::collections::BTreeMap;

    let mut grouped: BTreeMap<String, Vec<FileBrowserItem>> = BTreeMap::new();
    for file in files {
        let folder = top_level_folder(&file.relative_path);
        grouped.entry(folder).or_default().push(file.clone());
    }

    let mut folders = grouped
        .into_iter()
        .map(|(path, mut files)| {
            files.sort_by(|a, b| {
                b.modified_at
                    .or(b.created_at)
                    .unwrap_or(0)
                    .cmp(&a.modified_at.or(a.created_at).unwrap_or(0))
                    .then_with(|| a.relative_path.cmp(&b.relative_path))
            });
            let document_count = files.len();
            files.truncate(8);
            FolderEntry {
                path,
                document_count,
                files,
            }
        })
        .collect::<Vec<_>>();
    folders.sort_by(|a, b| {
        if a.path == "daily" && b.path != "daily" {
            return std::cmp::Ordering::Less;
        }
        if b.path == "daily" && a.path != "daily" {
            return std::cmp::Ordering::Greater;
        }
        b.document_count
            .cmp(&a.document_count)
            .then_with(|| a.path.cmp(&b.path))
    });
    folders.truncate(80);
    folders
}

fn top_level_folder(relative_path: &str) -> String {
    relative_path
        .split('/')
        .next()
        .filter(|part| !part.is_empty() && *part != relative_path)
        .unwrap_or("/")
        .to_string()
}

fn daily_note_entries(files: &[BrowserFileRow]) -> Vec<DailyNoteEntry> {
    let mut entries = files
        .iter()
        .filter_map(|file| {
            if file.kind != "markdown" {
                return None;
            }
            let filename = filename_for(&file.path);
            let date = daily_note_date(&filename, &file.relative_path)?;
            let source = fs::read_to_string(&file.path).unwrap_or_default();
            let (frontmatter, _, _) = split_frontmatter(&source);
            let last_updated = frontmatter.as_ref().and_then(|value| {
                metadata_first_string(value, &["last_updated", "updated", "modified"])
            });
            let ai_processed_at = frontmatter.as_ref().and_then(|value| {
                metadata_first_string(
                    value,
                    &["ai_processed_at", "processed_at", "bertus_processed_at"],
                )
            });
            let ai_processed_status =
                daily_processed_status(&date, last_updated.as_deref(), ai_processed_at.as_deref());
            Some(DailyNoteEntry {
                date,
                id: file.id,
                filename,
                relative_path: file.relative_path.clone(),
                last_updated,
                ai_processed_at,
                ai_processed_status,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.date.cmp(&b.date));
    entries
}

fn today_items(files: &[FileBrowserItem]) -> Vec<FileBrowserItem> {
    let today = unix_timestamp() as u64 / 86_400;
    let mut items = files
        .iter()
        .filter(|file| {
            file.created_at
                .or(file.modified_at)
                .map(|timestamp| timestamp / 86_400 == today)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by_key(|file| std::cmp::Reverse(file.created_at.or(file.modified_at).unwrap_or(0)));
    items.truncate(40);
    items
}

fn timeline_items(files: &[FileBrowserItem]) -> Vec<FileBrowserItem> {
    let mut items = files.to_vec();
    items.sort_by_key(|file| std::cmp::Reverse(file.created_at.or(file.modified_at).unwrap_or(0)));
    items.truncate(120);
    items
}

#[derive(Debug, Clone, Copy)]
enum GroupKind {
    Entity,
    Project,
}

fn grouped_metadata_entries(conn: &Connection, kind: GroupKind) -> Result<Vec<VaultGroupEntry>> {
    let mut grouped = record_collection_groups(conn, kind)?;
    let mut statement = conn.prepare(
        r#"
        select d.title, d.relative_path, d.frontmatter_json, coalesce(f.modified_ns, 0)
        from documents d
        left join files f on f.relative_path = d.relative_path
        order by d.relative_path
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    for row in rows {
        let (title, relative_path, frontmatter_json, modified_ns) = row?;
        let frontmatter = frontmatter_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
        let mut names = match kind {
            GroupKind::Entity => frontmatter
                .as_ref()
                .map(|value| metadata_values(value, &["entity", "entities"]))
                .unwrap_or_default(),
            GroupKind::Project => frontmatter
                .as_ref()
                .map(|value| metadata_values(value, &["project", "projects"]))
                .unwrap_or_default(),
        };
        if matches!(kind, GroupKind::Project) {
            if let Some(project) = project_from_path(&relative_path) {
                names.push(project);
            }
        }

        for name in names.into_iter().filter(|name| !name.trim().is_empty()) {
            if matches!(kind, GroupKind::Entity) && looks_like_project_identifier(&name) {
                continue;
            }
            let entry = grouped
                .entry(name)
                .or_insert_with(|| (0, i64::MIN, String::new(), String::new()));
            entry.0 += 1;
            if modified_ns >= entry.1 {
                entry.1 = modified_ns;
                entry.2 = title.clone();
                entry.3 = relative_path.clone();
            }
        }
    }

    let mut entries = grouped
        .into_iter()
        .map(
            |(name, (count, _, latest_title, latest_relative_path))| VaultGroupEntry {
                name,
                count,
                latest_title,
                latest_relative_path,
            },
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });
    entries.truncate(80);
    Ok(entries)
}

fn record_collection_groups(
    conn: &Connection,
    kind: GroupKind,
) -> Result<std::collections::BTreeMap<String, (usize, i64, String, String)>> {
    let collection_path = match kind {
        GroupKind::Entity => "records/entities.jsonl",
        GroupKind::Project => "records/projects.jsonl",
    };
    let Some(file) = file_row_by_relative_path(conn, collection_path)? else {
        return Ok(std::collections::BTreeMap::new());
    };
    let source = fs::read_to_string(&file.absolute_path).unwrap_or_default();
    let modified_ns = file.modified_ns.unwrap_or(0);
    let mut grouped = std::collections::BTreeMap::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let Some(name) = record_display_name(&value, kind) else {
            continue;
        };
        let entry = grouped
            .entry(name.clone())
            .or_insert_with(|| (0, modified_ns, name.clone(), collection_path.to_string()));
        entry.0 += 1;
        entry.1 = modified_ns;
        entry.2 = name;
        entry.3 = collection_path.to_string();
    }
    Ok(grouped)
}

fn record_display_name(value: &serde_json::Value, kind: GroupKind) -> Option<String> {
    let keys: &[&str] = match kind {
        GroupKind::Entity => &[
            "name",
            "title",
            "label",
            "entity",
            "entity_id",
            "id",
            "slug",
        ],
        GroupKind::Project => &[
            "name",
            "title",
            "label",
            "project",
            "project_id",
            "id",
            "slug",
        ],
    };
    for key in keys {
        if let Some(value) = value.get(*key).and_then(|value| value.as_str()) {
            let value = value.trim();
            if !value.is_empty() {
                if matches!(kind, GroupKind::Entity) && looks_like_project_identifier(value) {
                    continue;
                }
                return Some(value.to_string());
            }
        }
    }
    None
}

fn looks_like_project_identifier(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    bytes.len() > 8
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
}

fn metadata_values(frontmatter: &serde_json::Value, keys: &[&str]) -> Vec<String> {
    let mut values = Vec::new();
    for key in keys {
        collect_metadata_value(frontmatter.get(*key), &mut values);
    }
    values.sort();
    values.dedup();
    values
}

fn collect_metadata_value(value: Option<&serde_json::Value>, output: &mut Vec<String>) {
    match value {
        Some(serde_json::Value::String(value)) => output.push(value.clone()),
        Some(serde_json::Value::Array(values)) => {
            for value in values {
                collect_metadata_value(Some(value), output);
            }
        }
        Some(value) if value.is_number() || value.is_boolean() => output.push(value.to_string()),
        _ => {}
    }
}

fn project_from_path(relative_path: &str) -> Option<String> {
    let mut parts = relative_path.split('/');
    let first = parts.next()?;
    if first.eq_ignore_ascii_case("projects") || first == "40_projects" {
        return parts.next().map(str::to_string);
    }
    None
}

fn metadata_first_string(frontmatter: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let mut values = Vec::new();
        collect_metadata_value(frontmatter.get(*key), &mut values);
        if let Some(value) = values.into_iter().find(|value| !value.trim().is_empty()) {
            return Some(value);
        }
    }
    None
}

fn daily_processed_status(
    date: &str,
    last_updated: Option<&str>,
    ai_processed_at: Option<&str>,
) -> DailyNoteProcessedStatus {
    if date < "2026-07-01" {
        return DailyNoteProcessedStatus::NotTracked;
    }
    let Some(last_updated) = last_updated else {
        return DailyNoteProcessedStatus::Missing;
    };
    let Some(ai_processed_at) = ai_processed_at else {
        return DailyNoteProcessedStatus::Missing;
    };
    let Some(last_updated) = parse_frontmatter_timestamp(last_updated) else {
        return DailyNoteProcessedStatus::Missing;
    };
    let Some(ai_processed_at) = parse_frontmatter_timestamp(ai_processed_at) else {
        return DailyNoteProcessedStatus::Missing;
    };
    if ai_processed_at >= last_updated {
        DailyNoteProcessedStatus::Processed
    } else {
        DailyNoteProcessedStatus::Outdated
    }
}

fn parse_frontmatter_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .ok()
}

fn iso_timestamp_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn is_valid_daily_date(date: &str) -> bool {
    daily_note_date(&format!("{date}.md"), &format!("daily/{date}.md")).is_some()
}

fn is_daily_note_relative_path(relative_path: &str) -> bool {
    relative_path
        .strip_prefix("daily/")
        .and_then(|filename| daily_note_date(filename, relative_path))
        .is_some()
}

fn canonical_path_for_new_file(root: &Path, path: &Path) -> Result<PathBuf> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolve {}", root.display()))?;
    let parent = path
        .parent()
        .with_context(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let parent = parent
        .canonicalize()
        .with_context(|| format!("resolve {}", parent.display()))?;
    if !parent.starts_with(&root) {
        anyhow::bail!("new file path is outside vault: {}", path.display());
    }
    Ok(parent.join(path.file_name().context("path has no filename")?))
}

fn upsert_frontmatter_field(source: &str, key: &str, value: &str) -> Result<String> {
    if let Some(stripped) = source.strip_prefix("---\n") {
        if let Some(close_index) = stripped.find("\n---") {
            let yaml = &stripped[..close_index];
            let rest = &stripped[(close_index + 4)..];
            let mut lines = yaml.lines().map(str::to_string).collect::<Vec<_>>();
            let replacement = format!("{key}: {value}");
            let mut replaced = false;
            for line in &mut lines {
                let trimmed = line.trim_start();
                if trimmed.starts_with(&format!("{key}:")) {
                    *line = replacement.clone();
                    replaced = true;
                    break;
                }
            }
            let mut updated = lines.join("\n");
            if !replaced {
                if !updated.is_empty() {
                    updated.push('\n');
                }
                updated.push_str(&replacement);
            }
            return Ok(format!("---\n{updated}\n---{rest}"));
        }
    }
    Ok(format!("---\n{key}: {value}\n---\n\n{source}"))
}

fn daily_note_date(filename: &str, relative_path: &str) -> Option<String> {
    if !relative_path.starts_with("daily/") || !filename.ends_with(".md") {
        return None;
    }
    let date = filename.strip_suffix(".md")?;
    if date.len() != 10 {
        return None;
    }
    let bytes = date.as_bytes();
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    if !date
        .chars()
        .enumerate()
        .all(|(index, character)| matches!(index, 4 | 7) || character.is_ascii_digit())
    {
        return None;
    }

    Some(date.to_string())
}

fn system_time_seconds(time: std::time::SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn system_time_nanos(time: std::time::SystemTime) -> Option<i64> {
    let nanos = time.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    i64::try_from(nanos).ok()
}

fn nanos_to_secs(nanos: i64) -> Option<u64> {
    u64::try_from(nanos / 1_000_000_000).ok()
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn timestamp_from_filename(filename: &str) -> Option<u64> {
    let digits = filename
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.len() >= 12 {
        digits[..12].parse().ok()
    } else if digits.len() >= 8 {
        digits[..8].parse().ok()
    } else {
        None
    }
}

fn format_file_snippet(
    relative_path: &str,
    kind: &str,
    size_bytes: i64,
    modified_ns: Option<i64>,
) -> String {
    let modified = modified_ns
        .and_then(nanos_to_secs)
        .map(|value| format!(" · modified {value}"))
        .unwrap_or_default();
    format!("{kind} · {size_bytes} bytes · {relative_path}{modified}")
}

fn snippet_for(body: &str) -> String {
    body.split_whitespace()
        .take(24)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{extract_wikilinks, render_markdown, split_frontmatter, WikiLink};

    #[test]
    fn parses_frontmatter_and_body() {
        let (frontmatter, error, body) = split_frontmatter("---\ntitle: Test\n---\n# Body\n");
        assert_eq!(frontmatter.unwrap()["title"].as_str(), Some("Test"));
        assert_eq!(error, None);
        assert_eq!(body, "# Body\n");
    }

    #[test]
    fn keeps_simple_metadata_when_frontmatter_has_unparseable_lines() {
        let (frontmatter, error, body) = split_frontmatter(
            "---\ntitle: [broken\nproject: alpha\nentity: [example]\n---\n# Body\n",
        );
        let frontmatter = frontmatter.unwrap();
        assert!(error.unwrap().contains("frontmatter parse failed"));
        assert_eq!(frontmatter["project"].as_str(), Some("alpha"));
        assert_eq!(frontmatter["entity"][0].as_str(), Some("example"));
        assert_eq!(body, "# Body\n");
    }

    #[test]
    fn renders_wikilinks_as_local_links() {
        let html = render_markdown(
            "Open [[target-slug|Target]] and [[Example × Topic]].",
            Path::new("."),
            Path::new("note.md"),
        );
        assert!(html.contains("mvv://open/target-slug"));
        assert!(html.contains(">Target</a>"));
        assert!(html.contains("mvv://open/Example%20%C3%97%20Topic"));
        assert!(html.contains(">Example × Topic</a>"));
    }

    #[test]
    fn renders_callouts_and_inline_tags_without_rewriting_headings_or_code() {
        let html = render_markdown(
            r#"# Real Heading

Paragraph with #reader/tag.

```text
#not-a-tag
```

> [!warning] Watch this
> Keep the callout visible.

> Ordinary quote.
"#,
            Path::new("."),
            Path::new("note.md"),
        );

        assert!(html.contains("<h1>Real Heading</h1>"));
        assert!(html.contains(r#"<span class="vault-tag">#reader/tag</span>"#));
        assert!(html.contains("#not-a-tag"));
        assert!(html.contains(r#"<aside class="callout callout-warning">"#));
        assert!(html.contains(r#"<p class="callout-title">Watch this</p>"#));
        assert!(html.contains("<blockquote>"));
        assert!(html.contains("Ordinary quote."));
    }

    #[test]
    fn extracts_wikilink_targets_and_labels() {
        assert_eq!(
            extract_wikilinks("[[alpha]] and [[beta|Beta Label]] and ![[media.png]]"),
            vec![
                WikiLink {
                    target: "alpha".to_string(),
                    label: "alpha".to_string(),
                },
                WikiLink {
                    target: "beta".to_string(),
                    label: "Beta Label".to_string(),
                }
            ]
        );
    }
}
