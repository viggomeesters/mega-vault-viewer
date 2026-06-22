# Security Policy

Mega Vault Viewer is a local-first desktop app that reads files from a user-selected vault path and builds local shadow indexes. Security reports should focus on issues that could expose private vault contents, mutate source files unexpectedly, execute untrusted content, or leak runtime state.

## Supported Versions

The project is pre-1.0. Security fixes are handled on the default development branch until public releases are established.

## Reporting A Vulnerability

Do not open a public issue for a vulnerability.

Use GitHub private vulnerability reporting when the repository is hosted on GitHub. If that is not available yet, contact the maintainers through the private channel listed in the repository profile or release notes.

Please include:

- A clear description of the issue.
- Steps to reproduce with synthetic or fixture data.
- The affected platform and version or commit.
- The impact, especially whether private vault files, runtime indexes, or write operations are involved.

## Data Handling Expectations

- Do not attach private vaults, client files, personal notes, or private screenshots to public reports.
- Use `fixtures/demo-vault` or a minimal synthetic reproduction.
- Runtime indexes are rebuildable caches and should not be treated as canonical data.

## Scope

In scope:

- Unexpected file mutation or deletion.
- Path traversal or reading outside the selected vault when not intended.
- Untrusted content execution in rendered files.
- Leakage of vault paths, source text, metadata, or runtime indexes.

Out of scope:

- Vulnerabilities requiring a malicious local user with full filesystem access.
- Issues in third-party dependencies without a Mega Vault Viewer-specific exploit path.
