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
