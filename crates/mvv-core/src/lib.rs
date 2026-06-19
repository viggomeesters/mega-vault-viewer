use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, OwnedValue, Schema, TantivyDocument, FAST, INDEXED, STORED, STRING, TEXT,
};
use tantivy::{doc, Index};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultStats {
    pub documents: usize,
    pub links: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchHit {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub filename: String,
    pub stem: String,
    pub path: PathBuf,
    pub relative_path: String,
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

#[derive(Debug, Clone)]
pub struct VaultRuntime {
    root: PathBuf,
    db_path: PathBuf,
    index_dir: PathBuf,
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

impl VaultRuntime {
    pub fn build(root: impl AsRef<Path>, state_dir: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let state_dir = state_dir.as_ref().to_path_buf();
        fs::create_dir_all(&state_dir).context("create state directory")?;

        let db_path = state_dir.join("mega-vault-viewer.sqlite");
        let index_dir = state_dir.join("tantivy");
        if db_path.exists() {
            fs::remove_file(&db_path).context("clear sqlite index")?;
        }
        if index_dir.exists() {
            fs::remove_dir_all(&index_dir).context("clear tantivy index")?;
        }
        fs::create_dir_all(&index_dir).context("create tantivy index directory")?;

        let runtime = Self {
            root,
            db_path,
            index_dir,
        };
        runtime.rebuild()?;
        Ok(runtime)
    }

    pub fn stats(&self) -> Result<VaultStats> {
        let conn = self.open_db()?;
        let documents = conn.query_row("select count(*) from documents", [], |row| row.get(0))?;
        let links = conn.query_row("select count(*) from links", [], |row| row.get(0))?;
        Ok(VaultStats { documents, links })
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
        let query = parser.parse_query(query).context("parse search query")?;
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .context("search tantivy index")?;

        let conn = self.open_db()?;
        let mut hits = Vec::new();
        for (score, address) in top_docs {
            let retrieved = searcher.doc::<TantivyDocument>(address)?;
            let Some(id) = first_u64(&retrieved, fields.id) else {
                continue;
            };
            if let Some(summary) = document_summary(&conn, id as i64)? {
                hits.push(SearchHit {
                    id: summary.id,
                    slug: summary.slug,
                    title: summary.title,
                    filename: summary.filename,
                    stem: summary.stem,
                    path: summary.path,
                    relative_path: summary.relative_path,
                    snippet: snippet_for(&summary.body),
                    score,
                });
            }
        }
        Ok(hits)
    }

    pub fn open_by_slug(&self, slug: &str) -> Result<DocumentView> {
        let conn = self.open_db()?;
        let doc = conn
            .query_row(
                "select id, slug, title, filename, stem, path, relative_path, body, frontmatter_json, frontmatter_error from documents where slug = ?1 order by relative_path limit 1",
                [slug],
                row_to_document_view,
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
                row_to_document_view,
            )
            .optional()?
            .with_context(|| format!("document not found: {id}"))?;

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

    fn rebuild(&self) -> Result<()> {
        let mut conn = self.open_db()?;
        create_schema(&conn)?;
        let (index, fields) = create_search_index(&self.index_dir)?;
        let mut writer = index.writer(50_000_000).context("create tantivy writer")?;
        let tx = conn
            .transaction()
            .context("start sqlite index transaction")?;

        {
            let mut insert_document = tx.prepare(
                "insert into documents (slug, title, filename, stem, path, relative_path, body, frontmatter_json, frontmatter_error) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            let mut insert_link = tx.prepare(
                "insert into links (source_slug, target_slug, label) values (?1, ?2, ?3)",
            )?;

            for path in discover_markdown_paths(&self.root)? {
                let document = parse_markdown(&path, &self.root)?;
                insert_document.execute(params![
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
                ])?;
                let id = tx.last_insert_rowid();
                for link in &document.links {
                    insert_link.execute(params![document.slug, link.target, link.label])?;
                }
                writer.add_document(doc!(
                    fields.id => id as u64,
                    fields.slug => document.slug,
                    fields.title => document.title,
                    fields.filename => document.filename,
                    fields.relative_path => document.relative_path,
                    fields.body => document.body,
                ))?;
            }
        }

        tx.commit().context("commit sqlite index transaction")?;
        writer.commit().context("commit tantivy index")?;
        Ok(())
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
        create table documents (
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
        create table links (
          id integer primary key autoincrement,
          source_slug text not null,
          target_slug text not null,
          label text not null
        );
        create index documents_slug on documents(slug);
        create index documents_relative_path on documents(relative_path);
        create index links_source on links(source_slug);
        create index links_target on links(target_slug);
        "#,
    )?;
    Ok(())
}

fn discover_markdown_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }
        paths.push(path.to_path_buf());
    }
    paths.sort();
    Ok(paths)
}

fn parse_markdown(path: &Path, root: &Path) -> Result<IndexedDocument> {
    let source = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let (frontmatter, frontmatter_error, body) = split_frontmatter(&source);
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
            None,
            Some(format!("frontmatter parse failed: {error}")),
            body,
        ),
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

fn render_markdown(body: &str) -> String {
    let re = Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").expect("valid wikilink regex");
    let markdown = re.replace_all(body, |captures: &regex::Captures<'_>| {
        let target = captures.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let label = captures
            .get(2)
            .map(|m| m.as_str().trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(target);
        format!("[{label}](mvv://open/{target})")
    });
    catch_unwind_silent(|| {
        let parser = Parser::new_ext(&markdown, Options::all());
        let mut rendered = String::new();
        html::push_html(&mut rendered, parser);
        rendered
    })
    .unwrap_or_else(|_| render_plaintext_fallback(body))
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

fn search_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();
    let id = builder.add_u64_field("id", INDEXED | STORED | FAST);
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
        slug: schema.get_field("slug")?,
        title: schema.get_field("title")?,
        filename: schema.get_field("filename")?,
        relative_path: schema.get_field("relative_path")?,
        body: schema.get_field("body")?,
    })
}

fn first_u64(document: &TantivyDocument, field: Field) -> Option<u64> {
    document
        .get_first(field)
        .and_then(|value| match OwnedValue::from(value) {
            OwnedValue::U64(value) => Some(value),
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

fn row_to_document_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentView> {
    let body: String = row.get(7)?;
    let frontmatter_json: Option<String> = row.get(8)?;
    let frontmatter = frontmatter_json.and_then(|value| serde_json::from_str(&value).ok());
    Ok(DocumentView {
        id: row.get(0)?,
        slug: row.get(1)?,
        title: row.get(2)?,
        filename: row.get(3)?,
        stem: row.get(4)?,
        path: PathBuf::from(row.get::<_, String>(5)?),
        relative_path: row.get(6)?,
        html: render_markdown(&body),
        frontmatter,
        frontmatter_error: row.get(9)?,
        outgoing_links: Vec::new(),
        backlinks: Vec::new(),
    })
}

fn collect_strings(conn: &Connection, sql: &str, value: &str) -> Result<Vec<String>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([value], |row| row.get::<_, String>(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn snippet_for(body: &str) -> String {
    body.split_whitespace()
        .take(24)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{extract_wikilinks, render_markdown, split_frontmatter, WikiLink};

    #[test]
    fn parses_frontmatter_and_body() {
        let (frontmatter, error, body) = split_frontmatter("---\ntitle: Test\n---\n# Body\n");
        assert_eq!(frontmatter.unwrap()["title"].as_str(), Some("Test"));
        assert_eq!(error, None);
        assert_eq!(body, "# Body\n");
    }

    #[test]
    fn renders_wikilinks_as_local_links() {
        let html = render_markdown("Open [[target-slug|Target]].");
        assert!(html.contains("mvv://open/target-slug"));
        assert!(html.contains(">Target</a>"));
    }

    #[test]
    fn extracts_wikilink_targets_and_labels() {
        assert_eq!(
            extract_wikilinks("[[alpha]] and [[beta|Beta Label]]"),
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
