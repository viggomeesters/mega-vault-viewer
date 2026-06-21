use std::env;
use std::time::Instant;

use mvv_core::VaultRuntime;

fn main() -> anyhow::Result<()> {
    let vault = env::args()
        .nth(1)
        .expect("usage: cargo run -p mvv-core --example index_vault -- <vault-path> [state-dir]");
    let state = env::args()
        .nth(2)
        .map(Into::into)
        .unwrap_or_else(|| env::temp_dir().join("mega-vault-viewer-smoke-state"));
    let started = Instant::now();
    let runtime = VaultRuntime::build(&vault, &state)?;
    let stats = runtime.stats()?;
    let summary = runtime.index_summary();
    println!(
        "indexed {} documents and {} links in {:.2?}; scanned {}, updated {}, skipped {}, deleted {}, renamed {}, errored {}",
        stats.documents,
        stats.links,
        started.elapsed(),
        summary.scanned,
        summary.updated,
        summary.skipped,
        summary.deleted,
        summary.renamed,
        summary.errored
    );
    Ok(())
}
