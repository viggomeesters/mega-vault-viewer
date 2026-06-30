use std::path::PathBuf;

use mvv_core::VaultRuntime;

#[test]
#[ignore = "set MEGA_VAULT_VIEWER_MINIMAL_STARTER_PATH to a minimal-ai-vault-starter checkout"]
fn opens_real_minimal_ai_vault_starter_checkout() {
    let vault = std::env::var_os("MEGA_VAULT_VIEWER_MINIMAL_STARTER_PATH")
        .map(PathBuf::from)
        .expect("MEGA_VAULT_VIEWER_MINIMAL_STARTER_PATH is required");
    assert!(
        vault.join("docs/starter-contract.json").exists(),
        "minimal starter checkout must contain docs/starter-contract.json: {}",
        vault.display()
    );
    let state = tempfile::tempdir().unwrap();

    let runtime = VaultRuntime::build(&vault, state.path()).unwrap();
    let browser = runtime.file_browser().unwrap();
    let starter = browser
        .starter_vault
        .expect("minimal starter contract should be detected");

    assert_eq!(starter.name, "Minimal AI Vault Starter");
    assert!(starter.total_records >= 13);
    assert_eq!(starter.record_collections.len(), 8);
    assert!(starter
        .record_collections
        .iter()
        .any(|collection| collection.file == "records/sources.jsonl"
            && collection.schema.as_deref() == Some("schema/source.schema.json")));

    let daily = runtime
        .open_item_by_relative_path("vault/daily/2026-01-01.md")
        .unwrap();
    assert!(!daily.can_edit_source);

    let sources = runtime
        .open_item_by_relative_path("records/sources.jsonl")
        .unwrap();
    assert_eq!(sources.extension, "jsonl");
    assert!(sources
        .formatted
        .unwrap()
        .contains("source.daily.2026-01-01"));
}
