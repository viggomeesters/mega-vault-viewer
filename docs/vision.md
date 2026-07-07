# Vision Brief — Mega Vault Viewer

## North Star
Mega Vault Viewer is the calm, local-first read/view/search surface for filesystem knowledge vaults where source files stay canonical and runtime indexes are disposable.

## Wedge
A high-signal vault viewer for large Markdown/Obsidian-style vaults, not another editor-first Obsidian clone.

## Target User / Audience
Power users and agents working with large local vaults who need fast navigation, reliable daily note browsing, graph/search context, and safe rendering without mutating the vault.

## Core Promise
Open a vault once, keep it remembered, browse the real top-level structure, jump through daily notes, and read rich Markdown/CAS media from canonical files without preview duplicates or noisy app chrome.

## Product Principles
- Source files remain the source of truth; SQLite/Tantivy/render state is rebuildable cache.
- Read-first before edit-first; writes are explicit, scoped, and transaction-minded.
- JSONL-vault conventions are first-class: daily notes live at `daily/YYYY-MM-DD.md`; binary payloads live in CAS at `blobs/sha256/<first2>/<hash>`.
- CAS blobs are lazy-render payloads, not normal sync/index inputs; regular sync must not traverse/hash the full `blobs/` tree.
- Navigation should mirror the vault's mental model: top-level folders first, details on demand.
- The app shell must be calm: independent sidebar/document scrolling, non-redundant headers, compact metadata.
- Local-first UX must remember the current vault and offer a switcher without cloud/account state.
- Verification must include fixture/runtime proof for real vault layouts, not only static UI assumptions.

## Non-Goals
- Not an Obsidian Live Preview replacement.
- Not a cloud sync, hosted account, or telemetry product.
- Not a second binary payload store or preview-file cache.
- Not a broad write automation system for human daily notes.
- Not a raw dump of every nested generated folder as primary navigation.

## Load-Bearing Assumptions
| Assumption | Fails if | Cheapest evidence | Kill / pivot criterion |
|---|---|---|---|
| Users want a viewer distinct from Obsidian editing | They keep returning to Obsidian for reading/searching because MVV adds friction | Real vault smoke with remembered path, daily navigation, readable note view | If viewer cannot beat Obsidian for read/search flow, narrow to diagnostics/index browser |
| JSONL-vault daily layout is stable | Daily notes move away from `daily/YYYY-MM-DD.md` | Fixture test asserting daily path discovery/opening | If multiple layouts become canonical, add explicit contract detection instead of hardcoding |
| CAS media can render without preview duplicates | Extensionless blobs cannot be safely MIME-sniffed/rendered | CAS regression test from `blobs/sha256/<first2>/<hash>` | If MIME sniffing is unreliable, add metadata lookup but still avoid duplicate payloads |
| Top-level folder navigation is enough as default | Users need deep folders immediately for common workflows | Screenshot/user test: sidebar shows `daily`, `records`, `blobs`, `manifests`, etc. not hundreds of campaigns | If deep browsing is needed, add expand-on-demand tree, not flat deep list |

## First Proof
The app opens a JSONL-vault shaped fixture or real vault, defaults to/latest-detects `daily/YYYY-MM-DD.md`, shows top-level folders, remembers/switches vaults, renders CAS blobs, shows vault size in GB, and keeps sidebar scrolling independent from the document pane. `npm run check` must pass.

## First Build Slices
1. Repo-local `.go` state + durable vision/tasks so future agents know the product constraints.
2. Core runtime: daily note detection from `daily/YYYY-MM-DD.md`, latest daily first item, top-level folder aggregation, vault size metric.
3. Desktop UX: remembered vault/local switcher, GB size display, independent scroll regions, cleaner title header.
4. Regression tests for JSONL-vault layout, CAS rendering, folder aggregation, and stats.
5. UI smoke against the desktop/web app with screenshot-facing checks for layout and scroll behavior.

## Open Questions
- Should deep folder browsing become an expandable tree in v2, or stay accessible via search/recent only?
- Should edit mode remain inside MVV long-term or move behind an explicit advanced/write gate?
- Should JSONL-vault contract detection read `contracts/vault-contract.json` for future variants?

## Next Planning Boundary
Use repo-local `.go` tasks for the current UI/runtime cleanup, then run repo-complete/public readiness separately after the product slice is green.
