use std::env;
use std::time::Instant;

use mvv_core::VaultRuntime;

fn main() -> anyhow::Result<()> {
    let vault = env::args()
        .nth(1)
        .expect("usage: cargo run -p mvv-core --example index_vault -- <vault-path>");
    let state = env::temp_dir().join("mega-vault-viewer-smoke-state");
    let started = Instant::now();
    let runtime = VaultRuntime::build(&vault, &state)?;
    let stats = runtime.stats()?;
    println!(
        "indexed {} documents and {} links in {:.2?}",
        stats.documents,
        stats.links,
        started.elapsed()
    );
    Ok(())
}
