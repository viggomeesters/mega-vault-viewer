import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type VaultStats = {
  documents: number;
  links: number;
};

type FrontmatterValue = null | string | number | boolean | FrontmatterValue[] | { [key: string]: FrontmatterValue };

type SearchHit = {
  id: number;
  slug: string;
  title: string;
  filename: string;
  stem: string;
  path: string;
  relative_path: string;
  snippet: string;
  score: number;
};

type DocumentView = {
  id: number;
  slug: string;
  title: string;
  filename: string;
  stem: string;
  path: string;
  relative_path: string;
  html: string;
  frontmatter: Record<string, FrontmatterValue> | null;
  frontmatter_error: string | null;
  outgoing_links: string[];
  backlinks: string[];
};

type FileBrowserItem = {
  id: number;
  slug: string;
  title: string;
  filename: string;
  relative_path: string;
  modified_at: number | null;
  created_at: number | null;
};

type FolderEntry = {
  path: string;
  document_count: number;
  files: FileBrowserItem[];
};

type DailyNoteEntry = {
  date: string;
  id: number;
  filename: string;
  relative_path: string;
};

type FileBrowserSnapshot = {
  folders: FolderEntry[];
  newest_files: FileBrowserItem[];
  recent_files: FileBrowserItem[];
  daily_notes: DailyNoteEntry[];
};

type IndexSnapshot = {
  stats: VaultStats;
  first_document: DocumentView | null;
};

type RefreshSnapshot = {
  stats: VaultStats;
};

type AppMode = "setup" | "indexing" | "ready" | "error";
type FileViewMode = "folders" | "newest" | "recent";
type CalendarDay = {
  date: Date;
  key: string;
  inMonth: boolean;
};

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("Missing app root");
}

const appRoot = app;

let currentDocument: DocumentView | null = null;
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
let backStack: number[] = [];
let forwardStack: number[] = [];
let currentSearchQuery = "";
let isRefreshing = false;
let lastRefreshAt: Date | null = null;
let refreshTimer: number | null = null;

const AUTO_REFRESH_MS = 10 * 60 * 1000;

const formatScore = new Intl.NumberFormat("en", {
  maximumFractionDigits: 2,
});

function render() {
  appRoot.innerHTML = `
    <section class="shell">
      <aside class="sidebar" aria-label="Vault controls">
        <div class="brand">
          <p class="eyebrow">Local-first</p>
          <h1>Mega Vault Viewer</h1>
        </div>

        ${renderVaultSetup()}
        ${renderDailyCalendar()}

        <label class="field">
          <span>Search</span>
          <input id="search-box" name="search" value="${escapeAttribute(currentSearchQuery)}" placeholder="Search title, body, slug" spellcheck="false" />
        </label>

        <div class="results" aria-label="Search results">
          ${renderSidebarExplorer()}
        </div>
      </aside>

      <section class="document-pane" aria-label="Current document">
        <header class="document-header">
          <div>
            <p class="status">${escapeHtml(statusText)}</p>
            <h2>${escapeHtml(currentDocument?.filename ?? "No document open")}</h2>
            ${currentDocument ? `<p class="document-title">${escapeHtml(currentDocument.title)}</p>` : ""}
          </div>
          <div class="document-actions">
            <div class="nav-buttons" aria-label="Document navigation">
              <button id="back-button" type="button" title="Back" aria-label="Back" ${backStack.length === 0 ? "disabled" : ""}>&lt;</button>
              <button id="forward-button" type="button" title="Forward" aria-label="Forward" ${forwardStack.length === 0 ? "disabled" : ""}>&gt;</button>
            </div>
            <code title="${escapeAttribute(currentDocument?.path ?? "mvv://local")}">${escapeHtml(currentDocument?.relative_path ?? "mvv://local")}</code>
          </div>
        </header>

        ${currentDocument ? renderMetadataPanel(currentDocument) : ""}
        ${currentDocument ? renderLinkPanel(currentDocument) : ""}

        <article class="document-body">
          ${
            currentDocument
              ? currentDocument.html
              : `<div class="empty-state"><h3>Start with the fixture vault</h3><p>The MVP indexes local Markdown, stores graph metadata in SQLite, and searches body text with Tantivy.</p></div>`
          }
        </article>
      </section>
    </section>
  `;

  bindEvents();
}

function renderMetadataPanel(document: DocumentView) {
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

function renderLinkPanel(document: DocumentView) {
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

function metadataSummary(document: DocumentView) {
  if (document.frontmatter_error) {
    return "parse issue";
  }
  if (!document.frontmatter || Object.keys(document.frontmatter).length === 0) {
    return "none";
  }

  return `${Object.keys(document.frontmatter).length} fields`;
}

function renderMetadataRows(document: DocumentView) {
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
    <button class="result" type="button" data-doc-id="${hit.id}">
      <strong title="${escapeAttribute(hit.filename)}">${escapeHtml(hit.filename)}</strong>
      <em title="${escapeAttribute(hit.relative_path)}">${escapeHtml(hit.relative_path)}</em>
      <span>${escapeHtml(hit.snippet)}</span>
      <small>${escapeHtml(hit.title)} · ${escapeHtml(hit.slug)} · ${formatScore.format(hit.score)}</small>
    </button>
  `;
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
  const classes = [
    "calendar-day",
    day.inMonth ? "" : "is-outside",
    isToday(day.date) ? "is-today" : "",
    isCurrentDocumentDailyNote(dailyNote) ? "is-selected" : "",
    dailyNote ? "has-note" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return `
    <button class="${classes}" type="button" data-calendar-date="${day.key}" ${dailyNote ? `data-doc-id="${dailyNote.id}"` : ""}>
      <span>${day.date.getDate()}</span>
      ${dailyNote ? `<i aria-hidden="true"></i>` : ""}
    </button>
  `;
}

function renderSidebarExplorer() {
  const hasSearch = currentSearchQuery.trim().length > 0;
  return `
    ${
      hasSearch
        ? `<section class="sidebar-section"><h3>Search results</h3>${searchResults.map(renderSearchHit).join("") || `<p class="empty">No results.</p>`}</section>`
        : ""
    }
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

function renderFileTab(mode: FileViewMode, label: string) {
  return `
    <button class="file-tab ${fileViewMode === mode ? "is-active" : ""}" type="button" data-file-view="${mode}" role="tab" aria-selected="${fileViewMode === mode}">
      ${escapeHtml(label)}
    </button>
  `;
}

function renderFileViewContent() {
  if (!fileBrowserSnapshot) {
    return `<p class="empty">Index a vault to browse files.</p>`;
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
    <button class="file-item" type="button" data-doc-id="${file.id}" title="${escapeAttribute(file.relative_path)}">
      <strong>${escapeHtml(file.filename)}</strong>
      <span>${escapeHtml(file.relative_path)}</span>
    </button>
  `;
}

function renderVaultSetup() {
  if (appMode === "ready" && !showVaultSetup) {
    return `
      <section class="vault-summary" aria-label="Current vault">
        <div>
          <span>Current vault</span>
          <strong title="${escapeAttribute(vaultPath)}">${escapeHtml(formatVaultName(vaultPath))}</strong>
          <small>${escapeHtml(formatVaultSummary())}</small>
        </div>
        <div class="compact-actions compact-actions-single">
          <button id="change-vault-button" class="secondary-button" type="button">Change</button>
        </div>
      </section>
    `;
  }

  return `
    <section class="setup-panel" aria-label="Vault setup">
      <label class="field">
        <span>Vault path</span>
        <input id="vault-path" name="vault-path" value="${escapeAttribute(vaultPath)}" spellcheck="false" ${appMode === "indexing" ? "disabled" : ""} />
      </label>

      <button id="index-button" type="button" ${appMode === "indexing" ? "disabled" : ""}>
        ${appMode === "indexing" ? "Indexing..." : currentStats ? "Reindex vault" : "Index vault"}
      </button>

      ${appMode === "indexing" ? `<div class="busy-state" role="status"><span class="spinner" aria-hidden="true"></span><span>Indexing vault in background</span></div>` : ""}
      ${appMode === "error" ? `<p class="error-text">${escapeHtml(lastError)}</p>` : ""}
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
  document.querySelector<HTMLButtonElement>("#change-vault-button")?.addEventListener("click", () => {
    showVaultSetup = true;
    render();
  });
  document.querySelector<HTMLInputElement>("#vault-path")?.addEventListener("input", (event) => {
    vaultPath = (event.target as HTMLInputElement).value;
  });
  document.querySelector<HTMLInputElement>("#search-box")?.addEventListener("keydown", (event) => {
    currentSearchQuery = (event.target as HTMLInputElement).value;
    if (event.key === "Enter") {
      void runSearch(currentSearchQuery);
    }
  });
  document.querySelector<HTMLInputElement>("#search-box")?.addEventListener("input", (event) => {
    currentSearchQuery = (event.target as HTMLInputElement).value;
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
      const id = Number(button.dataset.docId);
      if (Number.isFinite(id)) {
        void openDocumentById(id);
        return;
      }
      statusText = `No daily note for ${button.dataset.calendarDate}`;
      render();
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
  document.querySelectorAll<HTMLButtonElement>("[data-doc-id]").forEach((button) => {
    button.addEventListener("click", () => {
      if (button.dataset.calendarDate) {
        return;
      }
      const id = Number(button.dataset.docId);
      if (Number.isFinite(id)) {
        void openDocumentById(id);
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
  vaultPath = await invoke<string>("default_fixture_path");
  render();
}

async function indexVault() {
  try {
    stopAutoRefresh();
    appMode = "indexing";
    lastError = "";
    statusText = "Indexing vault in background...";
    render();
    const snapshot = await invoke<IndexSnapshot>("index_vault", { vaultPath });
    currentStats = snapshot.stats;
    currentDocument = snapshot.first_document;
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    backStack = [];
    forwardStack = [];
    statusText = `Indexed ${snapshot.stats.documents} documents`;
    searchResults = [];
    currentSearchQuery = "";
    lastRefreshAt = new Date();
    appMode = "ready";
    showVaultSetup = false;
    startAutoRefresh();
  } catch (error) {
    lastError = String(error);
    statusText = "Index failed";
    appMode = "error";
    showVaultSetup = true;
  }
  render();
}

async function refreshIndexInBackground(reason: "timer" | "focus" = "timer") {
  if (appMode !== "ready" || !vaultPath || isRefreshing) {
    return;
  }
  if (reason === "focus" && lastRefreshAt && Date.now() - lastRefreshAt.getTime() < AUTO_REFRESH_MS) {
    return;
  }

  const openPath = currentDocument?.relative_path ?? null;
  isRefreshing = true;
  statusText = "Updating index in background...";
  render();

  try {
    const snapshot = await invoke<RefreshSnapshot>("refresh_index", { vaultPath });
    currentStats = snapshot.stats;
    fileBrowserSnapshot = await invoke<FileBrowserSnapshot>("file_browser");
    lastRefreshAt = new Date();

    if (openPath) {
      currentDocument = await invoke<DocumentView>("open_document_by_path", { relativePath: openPath });
    }
    if (currentSearchQuery) {
      searchResults = await invoke<SearchHit[]>("search", { query: currentSearchQuery });
    }
    backStack = [];
    forwardStack = [];

    statusText = `Updated ${formatRefreshTime(lastRefreshAt)}`;
  } catch (error) {
    statusText = `Background update failed: ${String(error)}`;
  } finally {
    isRefreshing = false;
    render();
  }
}

async function runSearch(query: string) {
  try {
    currentSearchQuery = query;
    statusText = `Searching "${query}"…`;
    render();
    searchResults = await invoke<SearchHit[]>("search", { query });
    statusText = `${searchResults.length} result${searchResults.length === 1 ? "" : "s"}`;
  } catch (error) {
    statusText = String(error);
  }
  render();
}

async function openDocument(slug: string, recordHistory = true) {
  try {
    const document = await invoke<DocumentView>("open_document", { slug });
    applyOpenedDocument(document, `Opened ${document.filename}`, recordHistory);
  } catch (error) {
    statusText = String(error);
  }
  render();
}

async function openDocumentById(id: number, recordHistory = true) {
  try {
    const document = await invoke<DocumentView>("open_document_by_id", { id });
    applyOpenedDocument(document, `Opened ${document.filename}`, recordHistory);
  } catch (error) {
    statusText = String(error);
  }
  render();
}

async function navigateBack() {
  if (!currentDocument || backStack.length === 0) {
    return;
  }

  const id = backStack.pop();
  forwardStack.push(currentDocument.id);
  if (id !== undefined) {
    await openDocumentById(id, false);
  }
}

async function navigateForward() {
  if (!currentDocument || forwardStack.length === 0) {
    return;
  }

  const id = forwardStack.pop();
  backStack.push(currentDocument.id);
  if (id !== undefined) {
    await openDocumentById(id, false);
  }
}

function applyOpenedDocument(document: DocumentView, status: string, recordHistory: boolean) {
  if (recordHistory && currentDocument && currentDocument.id !== document.id) {
    backStack.push(currentDocument.id);
    forwardStack = [];
  }

  currentDocument = document;
  statusText = status;
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

function formatVaultName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) ?? path;
}

function formatStats(stats: VaultStats | null) {
  if (!stats) {
    return "Not indexed";
  }

  return `${stats.documents} docs, ${stats.links} links`;
}

function formatVaultSummary() {
  const stats = formatStats(currentStats);
  if (isRefreshing) {
    return `${stats} · syncing`;
  }
  if (lastRefreshAt) {
    return `${stats} · synced ${formatRefreshTime(lastRefreshAt)}`;
  }

  return stats;
}

function formatRefreshTime(date: Date) {
  return date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
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
