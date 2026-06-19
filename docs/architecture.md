# Architecture

## Read-First Runtime

The app treats source files and runtime indexes as separate layers.

- **Source layer:** Markdown files on disk. Human-readable and portable.
- **Structured runtime:** SQLite stores document ids, slugs, source paths, frontmatter-derived metadata, outgoing links and backlink queries.
- **Search runtime:** Tantivy stores full-text fields and shares SQLite document ids.
- **UI shell:** Tauri exposes local Rust commands to a small desktop interface.

## Source Formats vs Runtime Model

Markdown is the first source format because the existing vault and Obsidian ecosystem already use it. It is not the long-term runtime model. The runtime model is a canonical document record plus adapters:

```text
source file -> adapter -> canonical document -> SQLite graph/metadata + Tantivy text index -> UI/API
```

Future adapters can support PDF, Office, CSV/XLSX, JSON, HTML, images and transcripts without changing the graph/search contracts.

## Quality Gates

Schema validation belongs in the structured runtime. Life OS frontmatter rules can be loaded as contracts, evaluated per document and shown as non-blocking quality states. Repair should remain explicit and transactional.

Frontmatter is indexed as structured metadata instead of display text. The current UI keeps it collapsed by default, but the same model can later support:

- Filters by type, category, project, entity, topic, source and date fields.
- Validation warnings for missing or malformed Life OS fields.
- Schema quality gates before agent edits or imports are accepted.
- Agent evidence views that show which source fields justified a search result, summary or patch.

## Scale Path

The MVP does full rebuilds for clarity. The path to 1m/10m notes is:

1. Stable document ids and source file fingerprints.
2. Incremental SQLite updates by changed source path.
3. Tantivy segment updates tied to SQLite document ids.
4. Adapter-specific chunking for large and binary formats.
5. Agent API with provenance on every search/open/patch response.
