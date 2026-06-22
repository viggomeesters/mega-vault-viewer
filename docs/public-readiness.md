# Public Readiness Notes

This repository is intended to be safe for public GitHub publication.

## Scrub Scope

The public scrub checks source, docs, fixtures, package metadata, CI, and assets while excluding generated or third-party folders such as:

- `.git/`
- `node_modules/`
- `target/`
- `apps/desktop/dist/`

The scan looks for private keys, common token patterns, secret assignments, personal absolute paths, machine-local vault paths, and private organization/client references.

## Current Findings

- No tracked generated runtime index, build output, or dependency cache is part of the repository.
- Public visuals use fixture or synthetic data only.
- `.gitignore` excludes local environment files, runtime SQLite/WAL/SHM files, Tantivy indexes, logs, app bundles, and OS artifacts.
- A local absolute maintainer path was found in release documentation during the scrub and replaced with a placeholder command.

## Rule

Do not add private vault content, personal screenshots, customer/client data, local absolute paths, tokens, or generated runtime state to this repository. Use fixture data for examples, tests, screenshots, and issues.
