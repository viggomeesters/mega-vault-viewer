import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { headerEditToggleAction, headerEditToggleLabel, isEditDirty, type HeaderEditToggleState } from "./edit-session";
import "./styles.css";

type VaultStats = {
  documents: number;
  links: number;
  vault_size_bytes: number;
};

type IndexSummary = {
  scanned: number;
  skipped: number;
  updated: number;
  deleted: number;
  renamed: number;
  errored: number;
};

type FrontmatterValue = null | string | number | boolean | FrontmatterValue[] | { [key: string]: FrontmatterValue };

type SearchHit = {
  id: number | null;
  slug: string;
  title: string;
  filename: string;
  stem: string;
  path: string;
  relative_path: string;
  kind: string;
  extension: string;
  size_bytes: number;
  snippet: string;
  score: number;
};

type VaultItemView = {
  document_id: number | null;
  slug: string;
  title: string;
  filename: string;
  stem: string;
  path: string;
  relative_path: string;
  kind: string;
  extension: string;
  size_bytes: number;
  modified_at: number | null;
  html: string | null;
  formatted: string | null;
  source: string | null;
  media_data_url: string | null;
  media_mime: string | null;
  preview_message: string | null;
  frontmatter: Record<string, FrontmatterValue> | null;
  frontmatter_error: string | null;
  outgoing_links: string[];
  backlinks: string[];
  can_edit_source: boolean;
  can_open_system: boolean;
  error: string | null;
};

type FileBrowserItem = {
  id: number | null;
  document_id: number | null;
  slug: string;
  title: string;
  filename: string;
  relative_path: string;
  kind: string;
  extension: string;
  size_bytes: number;
  modified_at: number | null;
  created_at: number | null;
};

type FolderEntry = {
  path: string;
  document_count: number;
  files: FileBrowserItem[];
};

type DailyNoteProcessedStatus = "not_tracked" | "missing" | "outdated" | "processed";

type DailyNoteEntry = {
  date: string;
  id: number | null;
  filename: string;
  relative_path: string;
  last_updated: string | null;
  ai_processed_at: string | null;
  ai_processed_status: DailyNoteProcessedStatus;
};

type VaultGroupEntry = {
  name: string;
  count: number;
  latest_title: string;
  latest_relative_path: string;
};

type StarterRecordCollection = {
  file: string;
  schema: string | null;
  count: number;
  record_type: string | null;
  status: string;
};

type StarterVaultSummary = {
  name: string;
  promise: string;
  human_owned: string[];
  canonical: string[];
  generated: string[];
  record_collections: StarterRecordCollection[];
  total_records: number;
  human_note_count: number;
  generated_view_count: number;
};

type FileBrowserSnapshot = {
  folders: FolderEntry[];
  newest_files: FileBrowserItem[];
  recent_files: FileBrowserItem[];
  daily_notes: DailyNoteEntry[];
  today_items: FileBrowserItem[];
  timeline_items: FileBrowserItem[];
  entities: VaultGroupEntry[];
  projects: VaultGroupEntry[];
  starter_vault: StarterVaultSummary | null;
};

type IndexSnapshot = {
  stats: VaultStats;
  first_item: VaultItemView | null;
  index_summary: IndexSummary;
};

type RefreshSnapshot = {
  stats: VaultStats;
  index_summary: IndexSummary;
};

type WatchStatus = {
  watching: boolean;
  path: string | null;
};

type SaveSnapshot = {
  stats: VaultStats;
  item: VaultItemView;
};

type AppMode = "setup" | "indexing" | "ready" | "error";
type IndexHealth = "idle" | "watching" | "updating" | "stale" | "error";
type FileViewMode = "folders" | "newest" | "recent";
type CalendarDay = {
  date: Date;
  key: string;
  inMonth: boolean;
};

type OutlineEntry = {
  level: number;
  text: string;
};

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("Missing app root");
}

const appRoot = app;

let currentDocument: VaultItemView | null = null;
let currentStats: VaultStats | null = null;
let searchResults: SearchHit[] = [];
let fileBrowserSnapshot: FileBrowserSnapshot | null = null;
let fileViewMode: FileViewMode = "folders";
let calendarMonth = startOfMonth(new Date());
let statusText = "Ready";
let vaultPath = "";
let appMode: AppMode = "setup";
let showVaultSetup = true;
let lastError = "";
let backStack: string[] = [];
let forwardStack: string[] = [];
let searchInputValue = "";
let submittedSearchQuery = "";
let isSearchRunning = false;
let searchRequestId = 0;
let isRefreshing = false;
let isEditing = false;
let isSaving = false;
let isAutoSaving = false;
let editSource = "";
let loadedEditSource = "";
let editError = "";
let autosaveTimer: number | null = null;
let autosavePromise: Promise<void> | null = null;
let editSessionId = 0;
let lastRefreshAt: Date | null = null;
let refreshTimer: number | null = null;
let watchDebounceTimer: number | null = null;
let watchUnlisten: UnlistenFn | null = null;
let watchErrorUnlisten: UnlistenFn | null = null;
let indexHealth: IndexHealth = "idle";
let rememberedVaults: string[] = [];

const AUTO_REFRESH_MS = 10 * 60 * 1000;
const WATCH_DEBOUNCE_MS = 1200;
const EDIT_AUTOSAVE_DEBOUNCE_MS = 900;
const LAST_VAULT_STORAGE_KEY = "mega-vault-viewer:last-vault";
const RECENT_VAULTS_STORAGE_KEY = "mega-vault-viewer:recent-vaults";

const formatScore = new Intl.NumberFormat("en", {
  maximumFractionDigits: 2,
});

function render() {
  appRoot.innerHTML = `
    <section class="shell">
      <aside class="sidebar" aria-label="Vault explorer">
        <div class="brand compact-brand" aria-label="Mega Vault Viewer">
          <strong title="Mega Vault Viewer">MVV</strong>
          <span>${escapeHtml(formatIndexHealth())}</span>
        </div>

        ${renderVaultSetup()}
        <form class="query-search" id="search-form" role="search">
          <label class="field search-field" for="search-box">
            <span>Query</span>
            <input id="search-box" name="search" value="${escapeAttribute(searchInputValue)}" placeholder="Type query, press Enter" spellcheck="false" autocomplete="off" />
          </label>
          <button id="search-submit-button" class="secondary-button search-submit-button" type="submit" ${isSearchRunning ? "disabled" : ""}>${isSearchRunning ? "Searching" : "Search"}</button>
          <small class="search-hint">Enter runs search · typing stays local</small>
        </form>

        <div class="results" aria-label="Navigator results">
          ${renderSidebarExplorer()}
        </div>
      </aside>

      <section class="document-pane ${isEditing ? "is-editing" : ""}" aria-label="Current document">
        <p class="sr-only" aria-live="polite">${escapeHtml(statusText)}</p>
        <header class="document-header">
          <div class="document-heading">
            <h2>${escapeHtml(currentDocument?.filename ?? "No document open")}</h2>
            ${currentDocument ? `<code title="${escapeAttribute(currentDocument.path)}">${escapeHtml(currentDocument.relative_path)}</code>` : ""}
          </div>
          <div class="document-actions">
            <div class="document-action-row">
              ${
                currentDocument?.can_edit_source
                  ? `<button id="edit-toggle-button" class="secondary-button edit-toggle-button ${isEditing ? "is-active" : ""}" type="button" ${isSaving ? "disabled" : ""}>${escapeHtml(headerEditToggleLabel(currentHeaderEditToggleState()))}</button>`
                  : ""
              }
              ${
                currentDocument?.can_open_system
                  ? `<button id="open-system-button" class="secondary-button" type="button" ${isSaving ? "disabled" : ""}>Open</button>`
                  : ""
              }
              ${
                currentDocument?.can_edit_source && isEditing
                  ? `<div class="header-edit-actions" aria-label="Edit actions">
                      <button id="cancel-edit-button" class="secondary-button" type="button" ${isSaving ? "disabled" : ""}>Cancel</button>
                      <button id="save-edit-button" type="button" ${isSaving ? "disabled" : ""}>${isSaving ? "Saving..." : "Save"}</button>
                    </div>`
                  : ""
              }
              <div class="nav-buttons" aria-label="Document navigation">
                <button id="back-button" type="button" title="Back" aria-label="Back" ${backStack.length === 0 || isSaving ? "disabled" : ""}>&lt;</button>
                <button id="forward-button" type="button" title="Forward" aria-label="Forward" ${forwardStack.length === 0 || isSaving ? "disabled" : ""}>&gt;</button>
              </div>
            </div>
          </div>
        </header>

        ${currentDocument && !isEditing ? renderItemDetailsPanel(currentDocument) : ""}
        ${currentDocument && !isEditing && isMarkdownItem(currentDocument) ? renderMetadataPanel(currentDocument) : ""}
        ${currentDocument && !isEditing && isMarkdownItem(currentDocument) ? renderLinkPanel(currentDocument) : ""}

        <article class="document-body ${isEditing ? "is-editing" : ""}">
          ${renderDocumentContent()}
        </article>
      </section>

      <aside class="right-rail" aria-label="Calendar and outline">
        ${renderRightRail()}
      </aside>
    </section>
  `;

  bindEvents();
}

function renderItemDetailsPanel(item: VaultItemView) {
  return `
    <details class="metadata-panel item-details-panel">
      <summary>
        <span>Item</span>
        <small>${escapeHtml(item.kind)} · ${escapeHtml(formatBytes(item.size_bytes))}</small>
      </summary>
      <dl>
        ${renderMetadataRow("kind", item.kind)}
        ${renderMetadataRow("extension", item.extension || "none")}
        ${renderMetadataRow("size", formatBytes(item.size_bytes))}
        ${renderMetadataRow("modified", item.modified_at ? formatDateTime(item.modified_at) : "unknown")}
        ${renderMetadataRow("relative_path", item.relative_path)}
        ${renderMetadataRow("source_path", item.path)}
      </dl>
    </details>
  `;
}

function renderMetadataPanel(document: VaultItemView) {
  return `
    <details class="metadata-panel">
      <summary>
        <span>Frontmatter</span>
        <small>${escapeHtml(metadataSummary(document))}</small>
      </summary>
      <dl>
        ${renderMetadataRows(document)}
      </dl>
    </details>
  `;
}

function renderLinkPanel(document: VaultItemView) {
  const backlinks = document.backlinks.length;
  const outgoing = document.outgoing_links.length;
  return `
    <details class="link-panel">
      <summary>
        <span>Links</span>
        <small>${backlinks} back, ${outgoing} out</small>
      </summary>
      <div class="link-groups">
        <section>
          <strong>Backlinks</strong>
          <div>${document.backlinks.map(renderSlugButton).join("") || `<span class="empty-inline">None</span>`}</div>
        </section>
        <section>
          <strong>Outgoing</strong>
          <div>${document.outgoing_links.map(renderSlugButton).join("") || `<span class="empty-inline">None</span>`}</div>
        </section>
      </div>
    </details>
  `;
}

function renderDocumentContent() {
  if (!currentDocument) {
    return `<div class="empty-state"><h3>Start with the fixture vault</h3><p>The MVP indexes local Markdown, stores graph metadata in SQLite, and searches body text with Tantivy.</p></div>`;
  }
  if (currentDocument.error) {
    return renderPreviewState("Preview error", currentDocument.error, currentDocument.can_open_system);
  }
  if (!isEditing && currentDocument.html) {
    return currentDocument.html;
  }
  if (!isEditing && currentDocument.media_data_url) {
    return `<img class="vault-image item-image-preview" src="${escapeAttribute(currentDocument.media_data_url)}" alt="${escapeAttribute(currentDocument.filename)}" />`;
  }
  if (!isEditing && currentDocument.formatted !== null) {
    return renderStructuredInspector(currentDocument);
  }
  if (!isEditing) {
    return renderPreviewState(
      currentDocument.preview_message ?? "No inline preview is available for this file.",
      `${currentDocument.filename} can still be opened from the source file.`,
      currentDocument.can_open_system,
    );
  }

  return `
    <section class="editor-pane" aria-label="Markdown editor">
      <textarea id="note-editor" spellcheck="false" ${isSaving ? "disabled" : ""}>${escapeHtml(editSource)}</textarea>
      <div class="editor-footer">
        <div class="editor-meta">
          <p>${escapeHtml(currentDocument.relative_path)}</p>
          <small id="editor-save-state">${escapeHtml(editorSaveStateText())}</small>
        </div>
      </div>
      ${editError ? `<p class="error-text">${escapeHtml(editError)}</p>` : ""}
    </section>
  `;
}

function renderStructuredInspector(item: VaultItemView) {
  return `
    <section class="structured-inspector" aria-label="Structured file inspector">
      ${item.preview_message ? `<div class="large-preview-notice"><strong>Preview mode</strong><span>${escapeHtml(item.preview_message)}</span></div>` : ""}
      <pre><code>${escapeHtml(item.formatted ?? "")}</code></pre>
      ${
        item.source !== null
          ? `<details class="raw-source-panel"><summary>Raw source</summary><pre><code>${escapeHtml(item.source)}</code></pre></details>`
          : ""
      }
    </section>
  `;
}

function renderPreviewState(title: string, body: string, canOpenSystem: boolean) {
  return `
    <div class="empty-state file-preview-state">
      <h3>${escapeHtml(title)}</h3>
      <p>${escapeHtml(body)}</p>
      ${canOpenSystem ? `<button id="open-system-inline-button" type="button">Open in system</button>` : ""}
    </div>
  `;
}

function metadataSummary(document: VaultItemView) {
  if (document.frontmatter_error) {
    return "parse issue";
  }
  if (!document.frontmatter || Object.keys(document.frontmatter).length === 0) {
    return "none";
  }

  return `${Object.keys(document.frontmatter).length} fields`;
}

function renderMetadataRows(document: VaultItemView) {
  if (document.frontmatter_error) {
    return renderMetadataRow("error", document.frontmatter_error);
  }
  if (!document.frontmatter || Object.keys(document.frontmatter).length === 0) {
    return renderMetadataRow("status", "No frontmatter");
  }

  const priority = ["type", "category", "created", "timestamp", "slug", "source", "topics", "project", "entity", "aliases"];
  const keys = Object.keys(document.frontmatter);
  const orderedKeys = [
    ...priority.filter((key) => keys.includes(key)),
    ...keys.filter((key) => !priority.includes(key)).sort((a, b) => a.localeCompare(b)),
  ];

  return orderedKeys.map((key) => renderMetadataRow(key, document.frontmatter?.[key] ?? null)).join("");
}

function renderMetadataRow(key: string, value: FrontmatterValue) {
  return `
    <div>
      <dt>${escapeHtml(key)}</dt>
      <dd>${escapeHtml(formatFrontmatterValue(value))}</dd>
    </div>
  `;
}

function formatFrontmatterValue(value: FrontmatterValue): string {
  if (value === null) {
    return "";
  }
  if (Array.isArray(value)) {
    return value.map(formatFrontmatterValue).filter(Boolean).join(", ");
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }

  return String(value);
}

function renderSearchHit(hit: SearchHit) {
  return `
    <button class="result" type="button" data-relative-path="${escapeAttribute(hit.relative_path)}">
      <strong title="${escapeAttribute(hit.filename)}">${escapeHtml(hit.filename)}</strong>
      <em title="${escapeAttribute(hit.relative_path)}">${escapeHtml(hit.relative_path)}</em>
      <span>${escapeHtml(hit.snippet)}</span>
      <small>${escapeHtml(hit.kind)} · ${escapeHtml(hit.title)} · ${formatScore.format(hit.score)}</small>
    </button>
  `;
}

function renderRightRail() {
  if (appMode !== "ready") {
    return "";
  }
  return `
    ${renderDailyCalendar()}
    ${renderDocumentOutlinePanel()}
    ${renderStarterVaultSummary()}
  `;
}

function renderDocumentOutlinePanel() {
  if (!currentDocument) {
    return `
      <section class="document-outline-panel" aria-label="Document outline">
        <p class="outline-eyebrow">Outline</p>
        <p class="empty">Open a note to show chapters.</p>
      </section>
    `;
  }

  const entries = documentOutlineEntries(currentDocument);
  const body = entries.length > 0
    ? entries
        .map(
          (entry, index) => `
            <button class="outline-entry is-level-${entry.level}" type="button" data-outline-index="${index}">
              ${escapeHtml(entry.text)}
            </button>
          `,
        )
        .join("")
    : `<p class="empty">No headings yet.</p>`;

  return `
    <section class="document-outline-panel" aria-label="Document outline">
      <p class="outline-eyebrow">Outline</p>
      <strong title="${escapeAttribute(currentDocument.relative_path)}">${escapeHtml(currentDocument.filename)}</strong>
      <small>${escapeHtml(currentDocument.kind)} · ${escapeHtml(formatBytes(currentDocument.size_bytes))}</small>
      <div class="outline-list">${body}</div>
    </section>
  `;
}

function documentOutlineEntries(item: VaultItemView): OutlineEntry[] {
  if (item.html) {
    const entries: OutlineEntry[] = [];
    const headingPattern = /<h([1-6])[^>]*>([\s\S]*?)<\/h\1>/gi;
    for (const match of item.html.matchAll(headingPattern)) {
      const text = stripHtml(match[2]).trim();
      if (text) {
        entries.push({ level: Number(match[1]), text });
      }
    }
    if (entries.length > 0) {
      return entries.slice(0, 24);
    }
  }

  const text = item.source ?? item.formatted ?? "";
  const markdownEntries = text
    .split("\n")
    .map((line): OutlineEntry | null => {
      const match = /^(#{1,6})\s+(.+)$/.exec(line.trim());
      return match ? { level: match[1].length, text: match[2].replace(/[#`*_]/g, "").trim() } : null;
    })
    .filter((entry): entry is OutlineEntry => entry !== null)
    .slice(0, 24);
  if (markdownEntries.length > 0) {
    return markdownEntries;
  }

  if (["json", "jsonl", "yaml", "yml"].includes(item.extension)) {
    return [
      { level: 1, text: item.preview_message ? "Bounded preview" : "Structured data" },
      { level: 2, text: item.relative_path },
      { level: 2, text: item.source === null ? "Raw source not loaded" : "Raw source available" },
    ];
  }

  return [];
}

function stripHtml(value: string) {
  return value.replace(/<[^>]+>/g, "").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"');
}

function renderDailyCalendar() {
  const dailyNotes = dailyNotesByDate();
  const days = calendarDays(calendarMonth);
  return `
    <section class="daily-calendar" aria-label="Daily notes calendar">
      <div class="calendar-toolbar">
        <button class="calendar-icon-button" type="button" data-calendar-action="previous" title="Previous month" aria-label="Previous month">&lt;</button>
        <strong>${escapeHtml(formatMonth(calendarMonth))}</strong>
        <button class="calendar-text-button" type="button" data-calendar-action="today">Today</button>
        <button class="calendar-icon-button" type="button" data-calendar-action="next" title="Next month" aria-label="Next month">&gt;</button>
      </div>
      <div class="calendar-weekdays" aria-hidden="true">
        ${["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((day) => `<span>${day}</span>`).join("")}
      </div>
      <div class="calendar-grid">
        ${days.map((day) => renderCalendarDay(day, dailyNotes.get(day.key))).join("")}
      </div>
    </section>
  `;
}

function renderCalendarDay(day: CalendarDay, dailyNote: DailyNoteEntry | undefined) {
  const processedStatus = dailyNote?.ai_processed_status ?? "missing";
  const classes = [
    "calendar-day",
    day.inMonth ? "" : "is-outside",
    isToday(day.date) ? "is-today" : "",
    isCurrentDocumentDailyNote(dailyNote) ? "is-selected" : "",
    dailyNote ? "has-note" : "",
    dailyNote ? `is-ai-${processedStatus}` : "is-missing-note",
  ]
    .filter(Boolean)
    .join(" ");
  const title = dailyNote
    ? `${dailyNote.relative_path} · AI ${processedStatus}${dailyNote.last_updated ? ` · updated ${dailyNote.last_updated}` : ""}${dailyNote.ai_processed_at ? ` · processed ${dailyNote.ai_processed_at}` : ""}`
    : `Create daily note ${day.key}`;

  return `
    <button class="${classes}" type="button" title="${escapeAttribute(title)}" data-calendar-date="${day.key}" ${dailyNote ? `data-relative-path="${escapeAttribute(dailyNote.relative_path)}"` : ""}>
      <span>${day.date.getDate()}</span>
      ${dailyNote ? `<i class="note-dot" aria-hidden="true"></i><i class="processed-dot" aria-hidden="true"></i>` : `<i class="create-dot" aria-hidden="true"></i>`}
    </button>
  `;
}

function renderSidebarExplorer() {
  if (submittedSearchQuery.trim().length > 0 || isSearchRunning) {
    const title = isSearchRunning ? "Searching…" : `Results for “${escapeHtml(submittedSearchQuery)}”`;
    const body = isSearchRunning
      ? `<p class="empty">Running search in the background.</p>`
      : searchResults.map(renderSearchHit).join("") || `<p class="empty">No results.</p>`;
    return `<section class="sidebar-section search-results-section"><h3>${title}</h3>${body}</section>`;
  }
  if (searchInputValue.trim().length > 0) {
    return `<section class="sidebar-section query-ready-section"><h3>Query ready</h3><p class="empty">Press Enter to search. Typing no longer searches every keystroke.</p></section>`;
  }
  return renderFilesSection();
}

function renderFilesSection() {
  return `
    <section class="file-viewer" aria-label="File viewer">
      <div class="file-viewer-header">
        <h3>Files</h3>
        <div class="file-tabs" role="tablist" aria-label="File views">
          ${renderFileTab("folders", "Folders")}
          ${renderFileTab("newest", "Newest")}
          ${renderFileTab("recent", "Recent")}
        </div>
      </div>
      ${renderFileViewContent()}
    </section>
  `;
}

function renderStarterVaultSummary() {
  const starter = fileBrowserSnapshot?.starter_vault;
  if (appMode !== "ready" || !starter) {
    return "";
  }
  return `
    <section class="starter-summary" aria-label="Minimal AI Vault Starter summary">
      <p class="eyebrow">Starter vault</p>
      <strong>${escapeHtml(starter.name)}</strong>
      <small>${starter.total_records} records · ${starter.human_note_count} human notes locked · ${starter.generated_view_count} generated views</small>
    </section>
  `;
}

function renderFileTab(mode: FileViewMode, label: string) {
  return `
    <button class="file-tab ${fileViewMode === mode ? "is-active" : ""}" type="button" data-file-view="${mode}" role="tab" aria-selected="${fileViewMode === mode}">
      ${escapeHtml(label)}
    </button>
  `;
}

function renderFileViewContent() {
  if (!fileBrowserSnapshot) {
    return `<p class="empty">Open a vault to browse files.</p>`;
  }
  if (fileViewMode === "folders") {
    return `
      <div class="folder-list">
        ${fileBrowserSnapshot.folders.map(renderFolderEntry).join("") || `<p class="empty">No folders.</p>`}
      </div>
    `;
  }

  const files = fileViewMode === "newest" ? fileBrowserSnapshot.newest_files : fileBrowserSnapshot.recent_files;
  return `
    <div class="file-list">
      ${files.map(renderFileItem).join("") || `<p class="empty">No files.</p>`}
    </div>
  `;
}

function renderFolderEntry(folder: FolderEntry) {
  return `
    <details class="folder-entry">
      <summary>
        <span>${escapeHtml(folder.path)}</span>
        <small>${folder.document_count}</small>
      </summary>
      <div class="folder-files">
        ${folder.files.map(renderFileItem).join("")}
      </div>
    </details>
  `;
}

function renderFileItem(file: FileBrowserItem) {
  return `
    <button class="file-item" type="button" data-relative-path="${escapeAttribute(file.relative_path)}" title="${escapeAttribute(file.relative_path)}">
      <strong>${escapeHtml(file.filename)}</strong>
      <span>${escapeHtml(file.kind)} · ${escapeHtml(file.relative_path)}</span>
    </button>
  `;
}

function renderVaultSetup() {
  const recentVaultButtons = renderRecentVaultButtons();
  if (appMode === "ready" && !showVaultSetup) {
    return `
      <section class="vault-chip" aria-label="Current vault">
        <button id="change-vault-button" class="vault-chip-main" type="button" title="${escapeAttribute(vaultPath)}">
          <strong>${escapeHtml(formatVaultName(vaultPath))}</strong>
          <small>${escapeHtml(formatStats(currentStats))}</small>
        </button>
      </section>
    `;
  }

  return `
    <section class="setup-panel compact-setup-panel" aria-label="Vault setup">
      <details class="vault-path-disclosure">
        <summary>
          <span>Vault</span>
          <code title="${escapeAttribute(vaultPath)}">${escapeHtml(formatVaultName(vaultPath) || "Choose vault")}</code>
        </summary>
        <label class="field compact-field">
          <span>Path</span>
          <input id="vault-path" name="vault-path" value="${escapeAttribute(vaultPath)}" placeholder="/path/to/vault" spellcheck="false" ${appMode === "indexing" ? "disabled" : ""} />
        </label>
      </details>

      ${recentVaultButtons}

      <button id="index-button" class="compact-sync-button" type="button" ${appMode === "indexing" ? "disabled" : ""}>
        ${appMode === "indexing" ? "Syncing..." : currentStats ? "Sync" : "Open"}
      </button>

      ${appMode === "indexing" ? `<div class="busy-state" role="status"><span class="spinner" aria-hidden="true"></span><span>Syncing vault in background</span></div>` : ""}
      ${appMode === "error" ? `<p class="error-text">${escapeHtml(lastError)}</p>` : ""}
    </section>
  `;
}

function renderRecentVaultButtons() {
  const vaults = rememberedVaults.filter((path) => path && path !== vaultPath).slice(0, 4);
  if (vaults.length === 0) {
    return "";
  }
  return `
    <section class="vault-switcher" aria-label="Recent vaults">
      <span>Recent vaults</span>
      <div>
        ${vaults
          .map(
            (path) => `
              <button class="vault-preset" type="button" data-vault-preset="${escapeAttribute(path)}" title="${escapeAttribute(path)}">
                <strong>${escapeHtml(formatVaultName(path))}</strong>
                <small>${escapeHtml(path)}</small>
              </button>
            `,
          )
          .join("")}
      </div>
    </section>
  `;
}

function renderSlugButton(slug: string) {
  return `<button class="slug-button" type="button" data-slug="${escapeAttribute(slug)}">${escapeHtml(slug)}</button>`;
}

function bindEvents() {
  document.querySelector<HTMLButtonElement>("#index-button")?.addEventListener("click", indexVault);
  document.querySelector<HTMLButtonElement>("#back-button")?.addEventListener("click", navigateBack);
  document.querySelector<HTMLButtonElement>("#forward-button")?.addEventListener("click", navigateForward);
  document.querySelector<HTMLButtonElement>("#edit-toggle-button")?.addEventListener("click", () => {
    const action = headerEditToggleAction(currentHeaderEditToggleState());
    if (action === "enter-edit") {
      void enterEditMode();
    }
    if (action === "save-and-read") {
      void saveEditMode();
    }
    if (action === "read") {
      leaveCleanEditMode();
    }
  });
  document.querySelector<HTMLButtonElement>("#open-system-button")?.addEventListener("click", () => {
    void openCurrentItemInSystem();
  });
  document.querySelector<HTMLButtonElement>("#open-system-inline-button")?.addEventListener("click", () => {
    void openCurrentItemInSystem();
  });
  document.querySelector<HTMLTextAreaElement>("#note-editor")?.addEventListener("input", (event) => {
    editSource = (event.target as HTMLTextAreaElement).value;
    editError = "";
    scheduleAutosave();
    refreshEditorSaveState();
  });
  document.querySelector<HTMLButtonElement>("#save-edit-button")?.addEventListener("click", () => {
    void saveEditMode();
  });
  document.querySelector<HTMLButtonElement>("#cancel-edit-button")?.addEventListener("click", cancelEditMode);
  document.querySelector<HTMLButtonElement>("#change-vault-button")?.addEventListener("click", () => {
    showVaultSetup = true;
    void stopWatchingVault();
    render();
  });
  document.querySelector<HTMLButtonElement>("#reset-index-button")?.addEventListener("click", () => {
    void resetIndex();
  });
  document.querySelector<HTMLInputElement>("#vault-path")?.addEventListener("input", (event) => {
    vaultPath = (event.target as HTMLInputElement).value;
  });
  document.querySelectorAll<HTMLButtonElement>("[data-vault-preset]").forEach((button) => {
    button.addEventListener("click", () => {
      const preset = button.dataset.vaultPreset;
      if (preset) {
        vaultPath = preset;
        void indexVault();
      }
    });
  });
  document.querySelector<HTMLFormElement>("#search-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const input = document.querySelector<HTMLInputElement>("#search-box");
    searchInputValue = input?.value ?? "";
    void runSearch(searchInputValue);
  });
  document.querySelector<HTMLInputElement>("#search-box")?.addEventListener("input", (event) => {
    searchInputValue = (event.target as HTMLInputElement).value;
    if (searchInputValue.trim().length === 0 && (submittedSearchQuery || searchResults.length > 0 || isSearchRunning)) {
      resetSearchState();
      render();
    }
  });
  document.querySelector<HTMLInputElement>("#search-box")?.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      searchInputValue = "";
      resetSearchState();
      render();
    }
  });
  document.querySelectorAll<HTMLButtonElement>("[data-outline-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const index = Number(button.dataset.outlineIndex ?? "-1");
      const headings = document.querySelectorAll<HTMLElement>(".document-body h1, .document-body h2, .document-body h3, .document-body h4, .document-body h5, .document-body h6");
      headings[index]?.scrollIntoView({ block: "start", behavior: "smooth" });
    });
  });
  document.querySelectorAll<HTMLButtonElement>("[data-calendar-action]").forEach((button) => {
    button.addEventListener("click", () => {
      const action = button.dataset.calendarAction;
      if (action === "previous") {
        calendarMonth = addMonths(calendarMonth, -1);
      }
      if (action === "next") {
        calendarMonth = addMonths(calendarMonth, 1);
      }
      if (action === "today") {
        calendarMonth = startOfMonth(new Date());
      }
      render();
    });
  });
  document.querySelectorAll<HTMLButtonElement>("[data-calendar-date]").forEach((button) => {
    button.addEventListener("click", () => {
      const relativePath = button.dataset.relativePath;
      if (relativePath) {
        void openItemByPath(relativePath);
        return;
      }
      const date = button.dataset.calendarDate;
      if (date) {
        void createOrOpenDailyNote(date);
      }
    });
  });
  document.querySelectorAll<HTMLButtonElement>("[data-file-view]").forEach((button) => {
    button.addEventListener("click", () => {
      const mode = button.dataset.fileView;
      if (mode === "folders" || mode === "newest" || mode === "recent") {
        fileViewMode = mode;
        render();
      }
    });
  });
  document.querySelectorAll<HTMLButtonElement>("[data-slug]").forEach((button) => {
    button.addEventListener("click", () => {
      const slug = button.dataset.slug;
      if (slug) {
        void openDocument(slug);
      }
    });
  });
  document.querySelectorAll<HTMLButtonElement>("[data-relative-path]").forEach((button) => {
    button.addEventListener("click", () => {
      if (button.dataset.calendarDate) {
        return;
      }
      const relativePath = button.dataset.relativePath;
      if (relativePath) {
        void openItemByPath(relativePath);
      }
    });
  });
  document.querySelector<HTMLElement>(".document-body")?.addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const link = target.closest<HTMLAnchorElement>('a[href^="mvv://open/"]');
    if (!link) {
      return;
    }
    event.preventDefault();
    const slug = decodeURIComponent(link.href.replace("mvv://open/", ""));
    void openDocument(slug);
  });
}

async function loadDefaultPath() {
  rememberedVaults = loadRememberedVaults();
  const storedVault = window.localStorage.getItem(LAST_VAULT_STORAGE_KEY) ?? "";
  try {
    const defaultVault = await invoke<string>("default_vault_path");
    vaultPath = storedVault || defaultVault;
  } catch (error) {
    vaultPath = storedVault;
    const message = String(error);
    if (!message.includes("invoke")) {
      lastError = message;
      appMode = storedVault ? "setup" : "error";
    }
  }
  render();
}

async function indexVault() {
  try {
    stopAutoRefresh();
    await stopWatchingVault();
    appMode = "indexing";
    indexHealth = "updating";
    lastError = "";
    statusText = "Syncing vault in background...";
    render();
    const snapshot = await invoke<IndexSnapshot>("index_vault", { vaultPath });
    currentStats = snapshot.stats;
    currentDocument = snapshot.first_item;
    rememberVault(vaultPath);
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    resetEditState();
    backStack = [];
    forwardStack = [];
    statusText = `Synced ${snapshot.stats.documents} documents · ${formatIndexSummary(snapshot.index_summary)}`;
    resetSearchState();
    lastRefreshAt = new Date();
    appMode = "ready";
    showVaultSetup = false;
    await startWatchingVault();
    startAutoRefresh();
  } catch (error) {
    lastError = String(error);
    statusText = "Sync failed";
    appMode = "error";
    indexHealth = "error";
    showVaultSetup = true;
  }
  render();
}

async function resetIndex() {
  try {
    stopAutoRefresh();
    await stopWatchingVault();
    appMode = "indexing";
    indexHealth = "updating";
    lastError = "";
    statusText = "Resetting rebuildable cache...";
    render();

    const snapshot = await invoke<IndexSnapshot>("reset_index", { vaultPath });
    currentStats = snapshot.stats;
    currentDocument = snapshot.first_item;
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    resetEditState();
    backStack = [];
    forwardStack = [];
    resetSearchState();
    lastRefreshAt = new Date();
    appMode = "ready";
    showVaultSetup = false;
    await startWatchingVault();
    startAutoRefresh();
    statusText = `Reset cache and synced ${snapshot.stats.documents} documents · ${formatIndexSummary(snapshot.index_summary)}`;
  } catch (error) {
    lastError = String(error);
    statusText = "Reset failed";
    appMode = "error";
    indexHealth = "error";
    showVaultSetup = true;
  }
  render();
}

async function startWatchingVault() {
  const status = await invoke<WatchStatus>("start_vault_watcher", { vaultPath });
  await installWatchListeners();
  indexHealth = status.watching ? "watching" : "idle";
}

async function stopWatchingVault() {
  clearWatchDebounce();
  if (watchUnlisten) {
    watchUnlisten();
    watchUnlisten = null;
  }
  if (watchErrorUnlisten) {
    watchErrorUnlisten();
    watchErrorUnlisten = null;
  }
  try {
    await invoke<WatchStatus>("stop_vault_watcher");
  } catch {
    // Watcher shutdown should not block opening a different vault.
  }
  indexHealth = "idle";
}

async function installWatchListeners() {
  if (watchUnlisten) {
    watchUnlisten();
  }
  if (watchErrorUnlisten) {
    watchErrorUnlisten();
  }
  watchUnlisten = await listen("vault_changed", () => {
    indexHealth = "stale";
    statusText = "Vault changed; syncing soon";
    scheduleWatchedRefresh();
    render();
  });
  watchErrorUnlisten = await listen<string>("vault_watch_error", (event) => {
    indexHealth = "error";
    statusText = `Watch failed: ${event.payload}`;
    render();
  });
}

function scheduleWatchedRefresh() {
  clearWatchDebounce();
  watchDebounceTimer = window.setTimeout(() => {
    watchDebounceTimer = null;
    void refreshIndexInBackground("watcher");
  }, WATCH_DEBOUNCE_MS);
}

function clearWatchDebounce() {
  if (watchDebounceTimer !== null) {
    window.clearTimeout(watchDebounceTimer);
    watchDebounceTimer = null;
  }
}

async function refreshIndexInBackground(reason: "timer" | "focus" | "watcher" = "timer") {
  if (appMode !== "ready" || !vaultPath || isRefreshing || isEditing) {
    return;
  }
  if (reason === "focus" && lastRefreshAt && Date.now() - lastRefreshAt.getTime() < AUTO_REFRESH_MS) {
    return;
  }

  const openPath = currentDocument?.relative_path ?? null;
  isRefreshing = true;
  indexHealth = "updating";
  statusText = "Syncing changes in background...";
  render();

  try {
    const snapshot = await invoke<RefreshSnapshot>("refresh_index", { vaultPath });
    currentStats = snapshot.stats;
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    lastRefreshAt = new Date();

    if (openPath) {
      currentDocument = await invoke<VaultItemView>("open_item_by_path", { relativePath: openPath });
    }
    if (submittedSearchQuery) {
      searchResults = await invoke<SearchHit[]>("search", { query: submittedSearchQuery });
    }
    backStack = [];
    forwardStack = [];

    indexHealth = "watching";
    statusText = `Updated ${formatRefreshTime(lastRefreshAt)} · ${formatIndexSummary(snapshot.index_summary)}`;
  } catch (error) {
    indexHealth = "error";
    statusText = `Background update failed: ${String(error)}`;
  } finally {
    isRefreshing = false;
    render();
  }
}

function resetSearchState() {
  searchResults = [];
  searchInputValue = "";
  submittedSearchQuery = "";
  isSearchRunning = false;
  searchRequestId += 1;
}

async function runSearch(rawQuery: string) {
  const query = rawQuery.trim();
  searchInputValue = rawQuery;
  if (query.length === 0) {
    resetSearchState();
    statusText = "Search cleared";
    render();
    return;
  }

  const requestId = searchRequestId + 1;
  searchRequestId = requestId;
  submittedSearchQuery = query;
  isSearchRunning = true;
  statusText = `Searching "${query}" in background…`;
  render();

  try {
    const results = await invoke<SearchHit[]>("search", { query });
    if (requestId !== searchRequestId) {
      return;
    }
    searchResults = results;
    statusText = `${results.length} result${results.length === 1 ? "" : "s"}`;
  } catch (error) {
    if (requestId !== searchRequestId) {
      return;
    }
    searchResults = [];
    statusText = String(error);
  } finally {
    if (requestId === searchRequestId) {
      isSearchRunning = false;
      render();
    }
  }
}

async function openDocument(slug: string, recordHistory = true) {
  try {
    const document = await invoke<VaultItemView>("open_document", { slug });
    applyOpenedDocument(document, `Opened ${document.filename}`, recordHistory);
  } catch (error) {
    statusText = String(error);
  }
  render();
}

async function openItemByPath(relativePath: string, recordHistory = true) {
  try {
    const item = await invoke<VaultItemView>("open_item_by_path", { relativePath });
    applyOpenedDocument(item, `Opened ${item.filename}`, recordHistory);
  } catch (error) {
    statusText = String(error);
  }
  render();
}

async function createOrOpenDailyNote(date: string) {
  try {
    statusText = `Opening daily note ${date}`;
    render();
    const snapshot = await invoke<SaveSnapshot>("create_or_open_daily_note", { vaultPath, date });
    currentStats = snapshot.stats;
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    applyOpenedDocument(snapshot.item, `Opened daily note ${date}`, true);
    lastRefreshAt = new Date();
    indexHealth = "watching";
    render();
  } catch (error) {
    statusText = String(error);
    render();
  }
}

async function openCurrentItemInSystem() {
  if (!currentDocument) {
    return;
  }
  try {
    await invoke<void>("open_item_in_system", { relativePath: currentDocument.relative_path });
    statusText = `Opened ${currentDocument.filename} in system`;
  } catch (error) {
    statusText = String(error);
  }
  render();
}

async function enterEditMode() {
  if (!currentDocument || isSaving) {
    return;
  }

  try {
    editError = "";
    statusText = `Loading source for ${currentDocument.filename}`;
    render();
    editSource = await invoke<string>("read_document_source", {
      relativePath: currentDocument.relative_path,
    });
    loadedEditSource = editSource;
    editSessionId += 1;
    isEditing = true;
    statusText = `Editing ${currentDocument.filename}`;
  } catch (error) {
    editError = String(error);
    statusText = "Edit failed";
  }
  render();
}

function leaveCleanEditMode() {
  if (!isEditing || isSaving) {
    return;
  }

  clearAutosaveTimer();
  resetEditState();
  statusText = currentDocument ? `Opened ${currentDocument.filename}` : "Ready";
  render();
}

function cancelEditMode() {
  if (!isEditing || isSaving) {
    return;
  }

  clearAutosaveTimer();
  resetEditState();
  statusText = currentDocument ? `Opened ${currentDocument.filename}` : "Ready";
  render();
}

async function saveEditMode() {
  if (!currentDocument || !isEditing || isSaving) {
    return;
  }

  clearAutosaveTimer();
  await waitForAutosave();

  if (!currentDocument || !isEditing || !isEditDirty(currentHeaderEditToggleState())) {
    leaveCleanEditMode();
    return;
  }

  const relativePath = currentDocument.relative_path;
  const source = editSource;
  try {
    isSaving = true;
    editError = "";
    statusText = `Saving ${currentDocument.filename}`;
    render();
    const snapshot = await persistEditSource(relativePath, source);
    currentStats = snapshot.stats;
    currentDocument = snapshot.item;
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    backStack = [];
    forwardStack = [];
    lastRefreshAt = new Date();
    indexHealth = "watching";
    loadedEditSource = source;
    resetEditState();
    statusText = `Saved ${currentDocument.filename}`;
  } catch (error) {
    editError = String(error);
    statusText = "Save failed";
  } finally {
    isSaving = false;
  }
  render();
}

async function navigateBack() {
  if (!currentDocument || backStack.length === 0) {
    return;
  }

  const relativePath = backStack.pop();
  forwardStack.push(currentDocument.relative_path);
  if (relativePath !== undefined) {
    await openItemByPath(relativePath, false);
  }
}

async function navigateForward() {
  if (!currentDocument || forwardStack.length === 0) {
    return;
  }

  const relativePath = forwardStack.pop();
  backStack.push(currentDocument.relative_path);
  if (relativePath !== undefined) {
    await openItemByPath(relativePath, false);
  }
}

function applyOpenedDocument(document: VaultItemView, status: string, recordHistory: boolean) {
  if (recordHistory && currentDocument && currentDocument.relative_path !== document.relative_path) {
    backStack.push(currentDocument.relative_path);
    forwardStack = [];
  }

  resetEditState();
  currentDocument = document;
  statusText = status;
  if (isDailyNotePath(document.relative_path) && document.can_edit_source) {
    window.setTimeout(() => {
      if (currentDocument?.relative_path === document.relative_path && !isEditing) {
        void enterEditMode();
      }
    }, 0);
  }
}

function resetEditState() {
  clearAutosaveTimer();
  editSessionId += 1;
  isEditing = false;
  isSaving = false;
  isAutoSaving = false;
  editSource = "";
  loadedEditSource = "";
  editError = "";
}

function currentHeaderEditToggleState(): HeaderEditToggleState {
  return {
    isEditing,
    isSaving,
    editSource,
    loadedSource: loadedEditSource,
  };
}

function refreshEditToggleLabel() {
  const button = document.querySelector<HTMLButtonElement>("#edit-toggle-button");
  if (button) {
    button.textContent = headerEditToggleLabel(currentHeaderEditToggleState());
  }
}

function refreshEditorSaveState() {
  refreshEditToggleLabel();
  const saveState = document.querySelector<HTMLElement>("#editor-save-state");
  if (saveState) {
    saveState.textContent = editorSaveStateText();
  }
}

function editorSaveStateText() {
  if (editError) {
    return "Autosave failed";
  }
  if (isSaving) {
    return "Saving...";
  }
  if (isAutoSaving) {
    return "Autosaving...";
  }
  if (autosaveTimer !== null) {
    return "Unsaved changes";
  }
  if (isEditDirty(currentHeaderEditToggleState())) {
    return "Unsaved changes";
  }

  return isEditing ? "Saved" : "";
}

function scheduleAutosave() {
  clearAutosaveTimer();
  if (!currentDocument || !isEditing || isSaving || !isEditDirty(currentHeaderEditToggleState())) {
    return;
  }

  autosaveTimer = window.setTimeout(() => {
    autosaveTimer = null;
    autosavePromise = autoSaveEdit();
    void autosavePromise.finally(() => {
      autosavePromise = null;
    });
  }, EDIT_AUTOSAVE_DEBOUNCE_MS);
}

function clearAutosaveTimer() {
  if (autosaveTimer !== null) {
    window.clearTimeout(autosaveTimer);
    autosaveTimer = null;
  }
}

async function waitForAutosave() {
  if (autosavePromise) {
    await autosavePromise;
  }
}

async function autoSaveEdit() {
  if (!currentDocument || !isEditing || isSaving || isAutoSaving || !isEditDirty(currentHeaderEditToggleState())) {
    return;
  }

  const sessionId = editSessionId;
  const relativePath = currentDocument.relative_path;
  const filename = currentDocument.filename;
  const source = editSource;

  try {
    isAutoSaving = true;
    editError = "";
    statusText = `Autosaving ${filename}`;
    refreshEditorSaveState();
    const snapshot = await persistEditSource(relativePath, source);
    if (sessionId !== editSessionId || !currentDocument || currentDocument.relative_path !== relativePath) {
      return;
    }

    currentStats = snapshot.stats;
    currentDocument = snapshot.item;
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    loadedEditSource = source;
    lastRefreshAt = new Date();
    indexHealth = "watching";
    statusText = editSource === loadedEditSource ? `Saved ${filename}` : `Editing ${filename}`;
  } catch (error) {
    if (sessionId === editSessionId) {
      editError = String(error);
      statusText = "Autosave failed";
    }
  } finally {
    if (sessionId === editSessionId) {
      isAutoSaving = false;
      refreshEditorSaveState();
      if (!editError && isEditDirty(currentHeaderEditToggleState())) {
        scheduleAutosave();
      }
    }
  }
}

function persistEditSource(relativePath: string, source: string) {
  return invoke<SaveSnapshot>("save_document_source", {
    vaultPath,
    relativePath,
    source,
  });
}

function isDailyNotePath(relativePath: string) {
  return /^daily\/\d{4}-\d{2}-\d{2}\.md$/.test(relativePath);
}

function startAutoRefresh() {
  stopAutoRefresh();
  refreshTimer = window.setInterval(() => {
    void refreshIndexInBackground("timer");
  }, AUTO_REFRESH_MS);
}

function stopAutoRefresh() {
  if (refreshTimer !== null) {
    window.clearInterval(refreshTimer);
    refreshTimer = null;
  }
}

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function escapeAttribute(value: string) {
  return escapeHtml(value).replaceAll("'", "&#39;");
}

function loadRememberedVaults() {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(RECENT_VAULTS_STORAGE_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === "string") : [];
  } catch {
    return [];
  }
}

function rememberVault(path: string) {
  const trimmed = path.trim();
  if (!trimmed) {
    return;
  }
  rememberedVaults = [trimmed, ...rememberedVaults.filter((vault) => vault !== trimmed)].slice(0, 8);
  window.localStorage.setItem(LAST_VAULT_STORAGE_KEY, trimmed);
  window.localStorage.setItem(RECENT_VAULTS_STORAGE_KEY, JSON.stringify(rememberedVaults));
}

function formatVaultName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) ?? path;
}

function formatStats(stats: VaultStats | null) {
  if (!stats) {
    return "Not synced";
  }

  return `${stats.documents} docs, ${stats.links} links, ${formatGigabytes(stats.vault_size_bytes)}`;
}

function formatIndexHealth() {
  if (isRefreshing || indexHealth === "updating") {
    return "updating";
  }
  if (indexHealth === "watching") {
    return "watching";
  }
  if (indexHealth === "stale") {
    return "stale";
  }
  if (indexHealth === "error") {
    return "error";
  }
  return "idle";
}

function formatIndexSummary(summary: IndexSummary) {
  const parts = [`${summary.updated} updated`, `${summary.skipped} skipped`];
  if (summary.deleted > 0) {
    parts.push(`${summary.deleted} deleted`);
  }
  if (summary.renamed > 0) {
    parts.push(`${summary.renamed} renamed`);
  }
  if (summary.errored > 0) {
    parts.push(`${summary.errored} errors`);
  }
  return `${summary.scanned} scanned, ${parts.join(", ")}`;
}

function formatRefreshTime(date: Date) {
  return date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDateTime(timestampSeconds: number) {
  return new Date(timestampSeconds * 1000).toLocaleString([], {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

function formatGigabytes(bytes: number) {
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unitIndex]}`;
}

function isMarkdownItem(item: VaultItemView) {
  return item.kind === "markdown";
}

function dailyNotesByDate() {
  const notes = new Map<string, DailyNoteEntry>();
  for (const note of fileBrowserSnapshot?.daily_notes ?? []) {
    notes.set(note.date, note);
  }
  return notes;
}

function calendarDays(month: Date): CalendarDay[] {
  const first = startOfMonth(month);
  const offset = (first.getDay() + 6) % 7;
  const start = new Date(first);
  start.setDate(first.getDate() - offset);

  return Array.from({ length: 42 }, (_, index) => {
    const date = new Date(start);
    date.setDate(start.getDate() + index);
    return {
      date,
      key: dateKey(date),
      inMonth: date.getMonth() === first.getMonth(),
    };
  });
}

function startOfMonth(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), 1);
}

function addMonths(date: Date, amount: number) {
  return new Date(date.getFullYear(), date.getMonth() + amount, 1);
}

function dateKey(date: Date) {
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`;
}

function pad2(value: number) {
  return String(value).padStart(2, "0");
}

function formatMonth(date: Date) {
  return date.toLocaleDateString([], {
    month: "short",
    year: "numeric",
  });
}

function isToday(date: Date) {
  return dateKey(date) === dateKey(new Date());
}

function isCurrentDocumentDailyNote(note: DailyNoteEntry | undefined) {
  return Boolean(note && currentDocument?.relative_path === note.relative_path);
}

void loadDefaultPath();

document.addEventListener("keydown", (event) => {
  if (!event.metaKey || event.shiftKey || event.altKey || event.ctrlKey) {
    return;
  }
  if (event.key === "[") {
    event.preventDefault();
    void navigateBack();
  }
  if (event.key === "]") {
    event.preventDefault();
    void navigateForward();
  }
});

window.addEventListener("focus", () => {
  void refreshIndexInBackground("focus");
});
