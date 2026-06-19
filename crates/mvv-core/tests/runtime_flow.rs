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

    let runtime = VaultRuntime::build(&vault, temp.path().join("state")).unwrap();

    let stats = runtime.stats().unwrap();
    assert_eq!(stats.documents, 2);
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

    let alpha = runtime.open_by_slug("20260617-0900-alpha").unwrap();
    let frontmatter = alpha.frontmatter.unwrap();
    assert_eq!(frontmatter["type"].as_str(), Some("reference"));
    assert_eq!(frontmatter["category"].as_str(), Some("tool"));
    assert_eq!(frontmatter["topics"][0].as_str(), Some("viewer"));
    assert_eq!(alpha.frontmatter_error, None);
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
