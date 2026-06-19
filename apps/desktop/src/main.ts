import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type VaultStats = {
  documents: number;
  links: number;
};

type SearchHit = {
  id: number;
  slug: string;
  title: string;
  snippet: string;
  score: number;
};

type DocumentView = {
  id: number;
  slug: string;
  title: string;
  path: string;
  html: string;
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
            <h2>${escapeHtml(currentDocument?.title ?? "No document open")}</h2>
          </div>
          <code>${escapeHtml(currentDocument?.slug ?? "mvv://local")}</code>
        </header>

        <article class="document-body">
          ${
            currentDocument
              ? currentDocument.html
              : `<div class="empty-state"><h3>Start with the fixture vault</h3><p>The MVP indexes local Markdown, stores graph metadata in SQLite, and searches body text with Tantivy.</p></div>`
          }
        </article>

        <footer class="link-strip">
          <div>
            <strong>Backlinks</strong>
            <span>${currentDocument?.backlinks.map(renderSlugButton).join("") || "None"}</span>
          </div>
          <div>
            <strong>Outgoing</strong>
            <span>${currentDocument?.outgoing_links.map(renderSlugButton).join("") || "None"}</span>
          </div>
        </footer>
      </section>
    </section>
  `;

  bindEvents();
}

function renderSearchHit(hit: SearchHit) {
  return `
    <button class="result" type="button" data-slug="${escapeAttribute(hit.slug)}">
      <strong>${escapeHtml(hit.title)}</strong>
      <span>${escapeHtml(hit.snippet)}</span>
      <small>${escapeHtml(hit.slug)} · ${formatScore.format(hit.score)}</small>
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

async function openDocument(slug: string) {
  try {
    currentDocument = await invoke<DocumentView>("open_document", { slug });
    statusText = `Opened ${slug}`;
  } catch (error) {
    statusText = String(error);
  }
  render();
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
