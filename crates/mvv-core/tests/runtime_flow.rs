use std::fs;
use std::path::Path;

use mvv_core::VaultRuntime;

#[test]
fn indexes_fixture_vault_links_and_searches_body_text() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let notes = vault.join("10_notes/2026-06");
    fs::create_dir_all(&notes).unwrap();

    fs::write(
        notes.join("20260617-0900-alpha.md"),
        r#"---
title: Alpha Note
slug: 20260617-0900-alpha
type: reference
category: tool
topics: [viewer, graph]
---

# Alpha Note

This note mentions the graphite runtime and links to [[20260617-0915-beta|Beta]].
"#,
    )
    .unwrap();

    fs::write(
        notes.join("20260617-0915-beta.md"),
        r#"---
title: Beta Note
slug: 20260617-0915-beta
type: reference
category: tool
---

# Beta Note

Backlinked content for the local viewer.
"#,
    )
    .unwrap();
    let daily = vault.join("daily");
    fs::create_dir_all(&daily).unwrap();
    fs::write(
        daily.join("2026-06-19.md"),
        r#"---
title: Daily
slug: 2026-06-19
---

# Daily
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    assert_eq!(runtime.index_summary().scanned, 3);
    assert_eq!(runtime.index_summary().updated, 3);

    let stats = runtime.stats().unwrap();
    assert_eq!(stats.documents, 3);
    assert_eq!(stats.links, 1);

    let hits = runtime.search("graphite runtime", 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].slug, "20260617-0900-alpha");
    assert_eq!(hits[0].filename, "20260617-0900-alpha.md");
    assert_eq!(hits[0].stem, "20260617-0900-alpha");
    assert_eq!(
        hits[0].relative_path,
        "10_notes/2026-06/20260617-0900-alpha.md"
    );
    assert!(hits[0].path.ends_with("20260617-0900-alpha.md"));

    let beta = runtime.open_by_slug("20260617-0915-beta").unwrap();
    assert_eq!(beta.filename, "20260617-0915-beta.md");
    assert_eq!(beta.stem, "20260617-0915-beta");
    assert_eq!(beta.relative_path, "10_notes/2026-06/20260617-0915-beta.md");
    assert_eq!(beta.backlinks, vec!["20260617-0900-alpha"]);
    assert!(beta.html.contains("Beta Note"));

    let beta_by_path = runtime
        .open_by_relative_path("10_notes/2026-06/20260617-0915-beta.md")
        .unwrap();
    assert_eq!(beta_by_path.id, beta.id);

    let alpha = runtime.open_by_slug("20260617-0900-alpha").unwrap();
    let frontmatter = alpha.frontmatter.unwrap();
    assert_eq!(frontmatter["type"].as_str(), Some("reference"));
    assert_eq!(frontmatter["category"].as_str(), Some("tool"));
    assert_eq!(frontmatter["topics"][0].as_str(), Some("viewer"));
    assert_eq!(alpha.frontmatter_error, None);

    let browser = runtime.file_browser().unwrap();
    assert!(browser
        .folders
        .iter()
        .any(|folder| folder.path == "10_notes" && folder.document_count == 2));
    assert_eq!(browser.folders[0].path, "daily");
    assert_eq!(browser.folders[0].document_count, 1);
    assert!(browser
        .newest_files
        .iter()
        .any(|file| file.filename == "20260617-0915-beta.md"));
    assert!(browser
        .recent_files
        .iter()
        .any(|file| file.relative_path == "10_notes/2026-06/20260617-0900-alpha.md"));
    assert!(browser.daily_notes.iter().any(|daily| {
        daily.date == "2026-06-19" && daily.relative_path == "daily/2026-06-19.md"
    }));
    assert_eq!(
        runtime.first_item().unwrap().unwrap().relative_path,
        "daily/2026-06-19.md"
    );
    assert!(runtime.stats().unwrap().vault_size_bytes > 0);
}

#[test]
fn manifest_records_file_kind_extension_size_mtime_and_hash() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(vault.join("media")).unwrap();
    fs::write(
        vault.join("alpha.md"),
        r#"---
title: Alpha
slug: alpha
---

# Alpha
"#,
    )
    .unwrap();
    fs::write(vault.join("settings.yaml"), "theme: quiet\n").unwrap();
    fs::write(vault.join("media/photo.png"), b"not really a png").unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let manifest = runtime.file_manifest().unwrap();

    assert_eq!(manifest.len(), 3);
    let markdown = manifest
        .iter()
        .find(|entry| entry.relative_path == "alpha.md")
        .unwrap();
    assert_eq!(markdown.kind, "markdown");
    assert_eq!(markdown.extension, "md");
    assert!(markdown.size_bytes > 0);
    assert!(markdown.modified_ns.is_some());
    assert_eq!(markdown.content_hash.len(), 64);
    assert_eq!(markdown.status, "indexed");

    let yaml = manifest
        .iter()
        .find(|entry| entry.relative_path == "settings.yaml")
        .unwrap();
    assert_eq!(yaml.kind, "yaml");
    assert_eq!(yaml.extension, "yaml");

    let image = manifest
        .iter()
        .find(|entry| entry.relative_path == "media/photo.png")
        .unwrap();
    assert_eq!(image.kind, "image");
    assert_eq!(image.extension, "png");
    assert_eq!(runtime.stats().unwrap().documents, 1);
}

#[test]
fn searches_and_opens_jsonl_records() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(vault.join("records")).unwrap();
    fs::write(
        vault.join("records/entities.jsonl"),
        r#"{"id":"entity-anne","name":"Anne Meesters","kind":"person"}
{"id":"entity-duco","name":"Duco Meesters","kind":"person"}
"#,
    )
    .unwrap();
    fs::write(
        vault.join("records/projects.jsonl"),
        r#"{"id":"2025-11-smart-data-platform","name":"Smart Data Platform"}
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();

    let hits = runtime.search("Duco", 10).unwrap();
    assert!(hits
        .iter()
        .any(|hit| hit.relative_path == "records/entities.jsonl" && hit.kind == "json"));
    let item = runtime
        .open_item_by_relative_path("records/entities.jsonl")
        .unwrap();
    assert_eq!(item.kind, "json");
    assert_eq!(item.extension, "jsonl");
    assert!(item.formatted.unwrap().contains("Anne Meesters"));

    let browser = runtime.file_browser().unwrap();
    assert!(browser
        .entities
        .iter()
        .any(|entry| entry.name == "Anne Meesters"));
    assert!(browser
        .projects
        .iter()
        .any(|entry| entry.name == "Smart Data Platform"));
    assert!(!browser
        .entities
        .iter()
        .any(|entry| entry.name == "2025-11-smart-data-platform"));
}

#[test]
fn sync_skips_cas_blob_payload_tree() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(vault.join("daily")).unwrap();
    fs::create_dir_all(vault.join("blobs/sha256/aa")).unwrap();
    fs::create_dir_all(vault.join(".obsidian/plugins/cas-blob-viewer")).unwrap();

    fs::write(
        vault.join("daily/2026-07-07.md"),
        r#"# Daily

![[cas-sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|500]]
"#,
    )
    .unwrap();
    for index in 0..50 {
        fs::write(
            vault.join(format!("blobs/sha256/aa/{index:064x}")),
            vec![index as u8; 1024],
        )
        .unwrap();
    }
    fs::write(
        vault.join(".obsidian/plugins/cas-blob-viewer/main.js"),
        "plugin bundle",
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();

    assert_eq!(runtime.index_summary().scanned, 1);
    assert_eq!(runtime.stats().unwrap().documents, 1);
    assert!(runtime
        .file_manifest()
        .unwrap()
        .iter()
        .all(|entry| !entry.relative_path.starts_with("blobs/")
            && !entry.relative_path.starts_with(".obsidian/")));
}

#[test]
fn runtime_state_uses_explicit_cache_dir_and_reset_never_touches_vault_files() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let state = temp.path().join("application-support-state");
    fs::create_dir_all(&vault).unwrap();
    fs::write(
        vault.join("alpha.md"),
        r#"---
title: Alpha
slug: alpha
---

# Alpha

Canonical vault content.
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    assert_eq!(runtime.stats().unwrap().documents, 1);
    assert!(state.join("mega-vault-viewer.sqlite").exists());
    assert!(state.join("tantivy").exists());
    assert!(!vault.join("mega-vault-viewer.sqlite").exists());
    assert!(!vault.join("mega-vault-viewer.sqlite-wal").exists());
    assert!(!vault.join("mega-vault-viewer.sqlite-shm").exists());
    assert!(!vault.join("tantivy").exists());

    fs::write(state.join("mega-vault-viewer.sqlite-wal"), "wal").unwrap();
    fs::write(state.join("mega-vault-viewer.sqlite-shm"), "shm").unwrap();
    fs::create_dir_all(state.join("render-cache")).unwrap();
    fs::write(state.join("render-cache/thumb.bin"), "cache").unwrap();

    VaultRuntime::reset_runtime_state(&state).unwrap();

    assert!(vault.join("alpha.md").exists());
    assert!(fs::read_to_string(vault.join("alpha.md"))
        .unwrap()
        .contains("Canonical vault content."));
    assert!(!state.join("mega-vault-viewer.sqlite").exists());
    assert!(!state.join("mega-vault-viewer.sqlite-wal").exists());
    assert!(!state.join("mega-vault-viewer.sqlite-shm").exists());
    assert!(!state.join("tantivy").exists());
    assert!(!state.join("render-cache").exists());
}

#[test]
fn incremental_reindex_preserves_existing_document_ids_and_adds_new_files() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    fs::write(
        vault.join("zeta.md"),
        r#"---
title: Zeta
slug: zeta
---

# Zeta

Existing searchable body.
"#,
    )
    .unwrap();

    let state = temp.path().join("state");
    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    let zeta = runtime.open_by_slug("zeta").unwrap();
    assert_eq!(runtime.index_summary().updated, 1);
    assert_eq!(runtime.index_summary().skipped, 0);
    assert_eq!(runtime.stats().unwrap().documents, 1);

    fs::write(
        vault.join("alpha.md"),
        r#"---
title: Alpha
slug: alpha
---

# Alpha

New searchable body.
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    let zeta_after_reindex = runtime.open_by_slug("zeta").unwrap();
    assert_eq!(runtime.index_summary().scanned, 2);
    assert_eq!(runtime.index_summary().updated, 1);
    assert_eq!(runtime.index_summary().skipped, 1);
    assert_eq!(zeta_after_reindex.id, zeta.id);
    assert_eq!(runtime.stats().unwrap().documents, 2);

    let hits = runtime.search("Alpha", 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].slug, "alpha");
}

#[test]
fn incremental_reindex_updates_changed_files_and_removes_deleted_files() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let note = vault.join("alpha.md");

    fs::write(
        &note,
        r#"---
title: Alpha
slug: alpha
---

# Alpha

BeforeOnly
"#,
    )
    .unwrap();

    let state = temp.path().join("state");
    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    let alpha = runtime.open_by_slug("alpha").unwrap();
    assert_eq!(runtime.index_summary().updated, 1);
    assert_eq!(runtime.search("BeforeOnly", 5).unwrap().len(), 1);

    fs::write(
        &note,
        r#"---
title: Alpha
slug: alpha
---

# Alpha

AfterOnly
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    assert_eq!(runtime.index_summary().updated, 1);
    assert_eq!(runtime.index_summary().skipped, 0);
    assert_eq!(runtime.open_by_slug("alpha").unwrap().id, alpha.id);
    assert_eq!(runtime.search("BeforeOnly", 5).unwrap().len(), 0);
    assert_eq!(runtime.search("AfterOnly", 5).unwrap().len(), 1);

    fs::remove_file(&note).unwrap();
    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    assert_eq!(runtime.index_summary().scanned, 0);
    assert_eq!(runtime.index_summary().deleted, 1);
    assert_eq!(runtime.stats().unwrap().documents, 0);
    assert_eq!(runtime.search("AfterOnly", 5).unwrap().len(), 0);
    assert!(runtime.open_by_slug("alpha").is_err());
}

#[test]
fn incremental_reindex_reports_renamed_files() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let original = vault.join("alpha.md");
    let renamed = vault.join("renamed-alpha.md");

    fs::write(
        &original,
        r#"---
title: Alpha
slug: alpha
---

# Alpha

RenameBody
"#,
    )
    .unwrap();

    let state = temp.path().join("state");
    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    assert_eq!(runtime.index_summary().updated, 1);
    fs::rename(&original, &renamed).unwrap();

    let runtime = VaultRuntime::build(&vault, &state).unwrap();
    assert_eq!(runtime.index_summary().scanned, 1);
    assert_eq!(runtime.index_summary().updated, 1);
    assert_eq!(runtime.index_summary().deleted, 1);
    assert_eq!(runtime.index_summary().renamed, 1);
    assert_eq!(
        runtime.open_by_slug("alpha").unwrap().relative_path,
        "renamed-alpha.md"
    );

    let manifest = runtime.file_manifest().unwrap();
    assert!(manifest
        .iter()
        .any(|entry| entry.relative_path == "alpha.md" && entry.status == "deleted"));
    assert!(manifest
        .iter()
        .any(|entry| entry.relative_path == "renamed-alpha.md" && entry.status == "indexed"));
}

#[test]
fn reads_and_writes_raw_document_source_by_relative_path() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let notes = vault.join("10_notes/2026-06");
    fs::create_dir_all(&notes).unwrap();
    let note_path = notes.join("20260617-0900-alpha.md");

    fs::write(
        &note_path,
        r#"---
title: Alpha Note
slug: 20260617-0900-alpha
---

# Alpha Note

Original body.
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let relative_path = "10_notes/2026-06/20260617-0900-alpha.md";
    let source = runtime
        .document_source_by_relative_path(relative_path)
        .unwrap();
    assert!(source.contains("title: Alpha Note"));
    assert!(source.contains("Original body."));

    let updated_source = source.replace("Original body.", "Updated body.");
    runtime
        .write_document_source_by_relative_path(relative_path, &updated_source)
        .unwrap();

    let written = fs::read_to_string(note_path).unwrap();
    assert!(written.contains("title: Alpha Note"));
    assert!(written.contains("Updated body."));
    assert!(!written.contains("Original body."));
}

#[test]
fn indexes_duplicate_slugs_without_aborting_the_vault() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(vault.join("a")).unwrap();
    fs::create_dir_all(vault.join("b")).unwrap();

    for folder in ["a", "b"] {
        fs::write(
            vault.join(folder).join("duplicate.md"),
            r#"---
title: Duplicate Note
slug: duplicate-note
---

# Duplicate Note

Same slug in two source paths.
"#,
        )
        .unwrap();
    }

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();

    let stats = runtime.stats().unwrap();
    assert_eq!(stats.documents, 2);
    let hits = runtime.search("source paths", 10).unwrap();
    assert_eq!(hits.len(), 2);
    assert_ne!(hits[0].relative_path, hits[1].relative_path);

    let opened = runtime.open_by_id(hits[0].id.unwrap()).unwrap();
    assert_eq!(opened.slug, "duplicate-note");
    assert_eq!(opened.relative_path, hits[0].relative_path);
}

#[test]
fn malformed_frontmatter_is_reported_without_breaking_rendering() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    fs::write(
        vault.join("broken.md"),
        r#"---
title: [broken
---

# Broken

The body still renders.
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let broken = runtime.open_by_slug("broken").unwrap();

    assert_eq!(broken.frontmatter, None);
    assert!(broken.frontmatter_error.unwrap().contains("frontmatter"));
    assert!(broken.html.contains("The body still renders."));
}

#[test]
fn renders_local_markdown_and_obsidian_images_safely() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let notes = vault.join("10_notes/2026-06");
    let media = vault.join("30_media/2026-06");
    fs::create_dir_all(&notes).unwrap();
    fs::create_dir_all(&media).unwrap();

    fs::write(media.join("pixel.png"), [0x89, b'P', b'N', b'G']).unwrap();
    fs::write(notes.join("local.jpg"), [0xff, 0xd8, 0xff, 0xd9]).unwrap();

    fs::write(
        notes.join("image-note.md"),
        r#"---
slug: image-note
---

# Image Note

![Relative](local.jpg)

![[pixel.png]]

![Missing](missing.png)
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let image_note = runtime.open_by_slug("image-note").unwrap();

    assert!(image_note.html.contains("src=\"data:image/jpeg;base64,"));
    assert!(image_note.html.contains("src=\"data:image/png;base64,"));
    assert!(image_note.html.contains("class=\"missing-media\""));
    assert!(image_note.html.contains("missing.png"));
    assert!(!image_note.html.contains("src=\"local.jpg\""));
}

#[test]
fn renders_cas_sha256_images_from_vault_blobs_without_preview_copies() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let notes = vault.join("daily");
    let hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let blob_dir = vault.join("blobs/sha256/aa");
    fs::create_dir_all(&notes).unwrap();
    fs::create_dir_all(&blob_dir).unwrap();
    fs::write(
        blob_dir.join(hash),
        [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'],
    )
    .unwrap();
    fs::write(
        notes.join("2026-07-05.md"),
        format!(
            r#"---
slug: cas-note
---

# CAS Note

![[cas-sha256-{hash}|500]]

![[file.sha256.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|missing]]
"#
        ),
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let note = runtime.open_by_slug("cas-note").unwrap();

    assert!(note.html.contains(r#"class="vault-image vault-image-cas""#));
    assert!(note.html.contains(
        r#"data-cas-sha256="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa""#
    ));
    assert!(note.html.contains("src=\"data:image/png;base64,"));
    assert!(note.html.contains(
        "Missing CAS blob: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    ));
    assert!(!vault.join("views/file_previews").exists());
}

#[test]
fn reader_fixture_pins_markdown_and_obsidian_rendering_semantics() {
    let temp = tempfile::tempdir().unwrap();
    let vault = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/reader-vault");

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let document = runtime.open_by_slug("reader-quality").unwrap();

    assert_eq!(document.title, "Reader Quality Fixture");
    assert_eq!(document.frontmatter.as_ref().unwrap()["type"], "reference");
    assert_eq!(document.frontmatter_error, None);
    assert_eq!(document.outgoing_links, vec!["target-note"]);

    let html = document.html;
    assert!(html.contains("<h1>Reader Quality Fixture</h1>"));
    assert!(html.contains("<h2>Tasks</h2>"));
    assert!(html.contains(r#"<span class="vault-tag">#inline-tag</span>"#));
    assert!(html.contains(r#"href="mvv://open/target-note""#));
    assert!(html.contains(">friendly WikiLink</a>"));
    assert!(html.contains("<code>inline code</code>"));
    assert!(html.contains(r#"type="checkbox""#));
    assert!(html.contains("Keep source available"));
    assert!(html.contains("<table>"));
    assert!(html.contains("<td>Readable columns</td>"));
    assert!(html.contains(r#"<code class="language-rust">"#));
    assert!(html.contains(r#"<aside class="callout callout-tip">"#));
    assert!(html.contains(r#"<p class="callout-title">Reader tip</p>"#));
    assert!(html.contains("<blockquote>"));
    assert!(html.contains("Plain blockquotes still render as ordinary quoted text."));
    assert!(html.contains(r#"class="vault-image""#));
    assert!(html.contains(r#"alt="Fixture image""#));
    assert!(html.contains(r#"alt="Embedded red pixel""#));
    assert!(!html.contains("![[red.png"));

    let edge = runtime
        .open_by_slug("20260621-1705-reader-frontmatter-edge")
        .unwrap();
    assert_eq!(edge.frontmatter, None);
    assert!(edge.frontmatter_error.unwrap().contains("frontmatter"));
    assert!(edge.html.contains("Broken Frontmatter Fixture"));
    assert!(edge
        .html
        .contains("Malformed frontmatter should be reported"));
}

#[test]
fn understands_minimal_ai_vault_starter_contract_and_protects_human_notes() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(vault.join("docs")).unwrap();
    fs::create_dir_all(vault.join("records")).unwrap();
    fs::create_dir_all(vault.join("schema")).unwrap();
    fs::create_dir_all(vault.join("vault/daily")).unwrap();
    fs::create_dir_all(vault.join("vault/inbox")).unwrap();
    fs::create_dir_all(vault.join("views/markdown")).unwrap();

    fs::write(
        vault.join("docs/starter-contract.json"),
        r#"{"name":"Minimal AI Vault Starter","promise":"Write daily/inbox notes; automation derives JSONL records without mutating human-owned notes.","human_owned":["vault/daily/**","vault/inbox/**"],"canonical":["records/*.jsonl","schema/*.schema.json"],"generated":["views/**","vault/ai-daily/**","vault/generated/**","dist/**"]}"#,
    )
    .unwrap();
    fs::write(vault.join("schema/source.schema.json"), "{}\n").unwrap();
    fs::write(vault.join("schema/task.schema.json"), "{}\n").unwrap();
    fs::write(
        vault.join("records/sources.jsonl"),
        "{\"id\":\"source.daily.2026-01-01\",\"type\":\"source\",\"source_path\":\"vault/daily/2026-01-01.md\"}\n",
    )
    .unwrap();
    fs::write(
        vault.join("records/tasks.jsonl"),
        "{\"id\":\"task.review\",\"type\":\"task\",\"title\":\"Review starter\"}\n",
    )
    .unwrap();
    fs::write(
        vault.join("vault/daily/2026-01-01.md"),
        "# Daily\nHuman-owned evidence.\n",
    )
    .unwrap();
    fs::write(
        vault.join("vault/inbox/example.md"),
        "# Inbox\nHuman-owned capture.\n",
    )
    .unwrap();
    fs::write(vault.join("views/markdown/open-loops.md"), "# Open loops\n").unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let browser = runtime.file_browser().unwrap();
    let starter = browser.starter_vault.unwrap();

    assert_eq!(starter.name, "Minimal AI Vault Starter");
    assert_eq!(starter.total_records, 2);
    assert_eq!(starter.human_note_count, 2);
    assert_eq!(starter.generated_view_count, 1);
    assert!(starter
        .record_collections
        .iter()
        .any(|collection| collection.file == "records/sources.jsonl"
            && collection.schema.as_deref() == Some("schema/source.schema.json")
            && collection.record_type.as_deref() == Some("source")
            && collection.count == 1));

    let daily = runtime
        .open_item_by_relative_path("vault/daily/2026-01-01.md")
        .unwrap();
    assert!(!daily.can_edit_source);
    assert!(runtime
        .write_document_source_by_relative_path("vault/daily/2026-01-01.md", "# Mutated\n")
        .unwrap_err()
        .to_string()
        .contains("human-owned contract"));

    let generated = runtime
        .open_item_by_relative_path("views/markdown/open-loops.md")
        .unwrap();
    assert!(generated.can_edit_source);
}

#[test]
fn opens_mixed_format_vault_items_without_rewriting_sources() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(vault.join("10_notes")).unwrap();
    fs::create_dir_all(vault.join("system")).unwrap();
    fs::create_dir_all(vault.join("30_media")).unwrap();
    fs::create_dir_all(vault.join("20_files")).unwrap();

    fs::write(
        vault.join("10_notes/mixed.md"),
        r#"---
title: Mixed Markdown
slug: mixed-markdown
entity: [viggo-meesters]
project: mega-vault-viewer
---

# Mixed Markdown

Markdown body.
"#,
    )
    .unwrap();
    fs::write(
        vault.join("system/settings.yaml"),
        "name: Mega\nflags:\n  - reader\n",
    )
    .unwrap();
    fs::write(
        vault.join("system/state.json"),
        r#"{"name":"Mega","count":2}"#,
    )
    .unwrap();
    fs::write(
        vault.join("system/events.jsonl"),
        "{\"event\":\"open\"}\n{\"event\":\"preview\"}\n",
    )
    .unwrap();
    fs::write(vault.join("30_media/pixel.png"), [0x89, b'P', b'N', b'G']).unwrap();
    fs::write(vault.join("20_files/sample.pdf"), b"%PDF-1.4\n").unwrap();
    fs::write(vault.join("20_files/readme.bin"), b"plain fallback").unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();
    let browser = runtime.file_browser().unwrap();
    assert!(browser
        .newest_files
        .iter()
        .any(|file| file.relative_path == "system/settings.yaml" && file.kind == "yaml"));
    assert!(browser
        .folders
        .iter()
        .any(|folder| folder.path == "system" && folder.document_count == 3));
    assert!(browser
        .today_items
        .iter()
        .any(|file| file.relative_path == "10_notes/mixed.md"));
    assert!(browser
        .timeline_items
        .iter()
        .any(|file| file.relative_path == "system/state.json"));
    assert!(browser
        .entities
        .iter()
        .any(|entry| entry.name == "viggo-meesters" && entry.count == 1));
    assert!(browser
        .projects
        .iter()
        .any(|entry| entry.name == "mega-vault-viewer" && entry.count == 1));

    let markdown = runtime
        .open_item_by_relative_path("10_notes/mixed.md")
        .unwrap();
    assert_eq!(markdown.kind, "markdown");
    assert_eq!(markdown.slug, "mixed-markdown");
    assert_eq!(markdown.document_id, Some(1));
    assert!(markdown.html.unwrap().contains("<h1>Mixed Markdown</h1>"));
    assert!(markdown.can_edit_source);

    let yaml = runtime
        .open_item_by_relative_path("system/settings.yaml")
        .unwrap();
    assert_eq!(yaml.kind, "yaml");
    assert!(yaml.source.as_ref().unwrap().contains("name: Mega"));
    assert!(yaml
        .formatted
        .as_ref()
        .unwrap()
        .contains("\"name\": \"Mega\""));
    assert!(!yaml.can_edit_source);

    let json = runtime
        .open_item_by_relative_path("system/state.json")
        .unwrap();
    assert_eq!(json.kind, "json");
    assert!(json.formatted.as_ref().unwrap().contains("\"count\": 2"));

    let jsonl = runtime
        .open_item_by_relative_path("system/events.jsonl")
        .unwrap();
    assert_eq!(jsonl.extension, "jsonl");
    assert!(jsonl
        .formatted
        .as_ref()
        .unwrap()
        .contains("\"event\": \"preview\""));

    let image = runtime
        .open_item_by_relative_path("30_media/pixel.png")
        .unwrap();
    assert_eq!(image.kind, "image");
    assert!(image
        .media_data_url
        .as_ref()
        .unwrap()
        .starts_with("data:image/png;base64,"));

    let pdf = runtime
        .open_item_by_relative_path("20_files/sample.pdf")
        .unwrap();
    assert_eq!(pdf.kind, "pdf");
    assert!(pdf.preview_message.unwrap().contains("PDF preview"));
    assert!(pdf.can_open_system);

    let generic = runtime
        .open_item_by_relative_path("20_files/readme.bin")
        .unwrap();
    assert_eq!(generic.kind, "file");
    assert_eq!(generic.source.as_deref(), Some("plain fallback"));

    let hits = runtime.search("settings.yaml", 10).unwrap();
    assert!(hits
        .iter()
        .any(|hit| hit.relative_path == "system/settings.yaml" && hit.kind == "yaml"));
}
