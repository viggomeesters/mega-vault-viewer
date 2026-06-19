use std::fs;

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
    fs::write(
        notes.join("20260619-0600-daily.md"),
        r#"---
title: Daily
slug: 20260619-0600-daily
---

# Daily
"#,
    )
    .unwrap();

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();

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
        .any(|folder| folder.path == "10_notes/2026-06" && folder.document_count == 3));
    assert!(browser
        .newest_files
        .iter()
        .any(|file| file.filename == "20260617-0915-beta.md"));
    assert!(browser
        .recent_files
        .iter()
        .any(|file| file.relative_path == "10_notes/2026-06/20260617-0900-alpha.md"));
    assert!(browser.daily_notes.iter().any(|daily| {
        daily.date == "2026-06-19"
            && daily.relative_path == "10_notes/2026-06/20260619-0600-daily.md"
    }));
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

    let opened = runtime.open_by_id(hits[0].id).unwrap();
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
