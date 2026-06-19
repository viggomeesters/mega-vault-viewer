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

type IndexSnapshot = {
  stats: VaultStats;
  first_document: DocumentView | null;
};

type AppMode = "setup" | "indexing" | "ready" | "error";

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("Missing app root");
}

const appRoot = app;

let currentDocument: DocumentView | null = null;
let currentStats: VaultStats | null = null;
let searchResults: SearchHit[] = [];
let statusText = "Ready";
let vaultPath = "";
let appMode: AppMode = "setup";
let showVaultSetup = true;
let lastError = "";
let backStack: number[] = [];
let forwardStack: number[] = [];

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

        <label class="field">
          <span>Search</span>
          <input id="search-box" name="search" placeholder="Search title, body, slug" spellcheck="false" />
        </label>

        <div class="results" aria-label="Search results">
          ${searchResults.map(renderSearchHit).join("") || `<p class="empty">Index a vault, then search.</p>`}
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

function renderVaultSetup() {
  if (appMode === "ready" && !showVaultSetup) {
    return `
      <section class="vault-summary" aria-label="Current vault">
        <div>
          <span>Current vault</span>
          <strong title="${escapeAttribute(vaultPath)}">${escapeHtml(formatVaultName(vaultPath))}</strong>
          <small>${escapeHtml(formatStats(currentStats))}</small>
        </div>
        <div class="compact-actions">
          <button id="reindex-button" class="secondary-button" type="button">Reindex</button>
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
  document.querySelector<HTMLButtonElement>("#reindex-button")?.addEventListener("click", indexVault);
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
    if (event.key === "Enter") {
      void runSearch((event.target as HTMLInputElement).value);
    }
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
    appMode = "indexing";
    lastError = "";
    statusText = "Indexing vault in background...";
    render();
    const snapshot = await invoke<IndexSnapshot>("index_vault", { vaultPath });
    currentStats = snapshot.stats;
    currentDocument = snapshot.first_document;
    backStack = [];
    forwardStack = [];
    statusText = `Indexed ${snapshot.stats.documents} documents`;
    searchResults = [];
    appMode = "ready";
    showVaultSetup = false;
  } catch (error) {
    lastError = String(error);
    statusText = "Index failed";
    appMode = "error";
    showVaultSetup = true;
  }
  render();
}

async function runSearch(query: string) {
  try {
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
