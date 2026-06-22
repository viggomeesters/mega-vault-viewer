# Visual Assets

Public visuals must be safe to publish. Do not use screenshots from a private vault, customer workspace, personal note collection, or machine-local path.

## Current Assets

- `assets/hero.svg` - README hero image.
- `assets/social-preview.svg` - GitHub/social preview candidate.
- `assets/screenshot-fixture-vault.svg` - representative app screenshot with synthetic fixture data.

## Refresh Workflow

1. Use `fixtures/demo-vault` or a tiny synthetic vault.
2. Avoid real personal names, client names, private paths, tokens, and screenshots from private notes.
3. Capture or generate a visual that shows the actual product shape: sidebar, indexed vault, rendered note, and local-first messaging.
4. Save final assets under `assets/`.
5. Run the privacy scan from the public readiness task before publishing.

For a future real screenshot, build or run the desktop app with fixture data:

```bash
MEGA_VAULT_VIEWER_DEFAULT_VAULT_PATH="$PWD/fixtures/demo-vault" npm run desktop:dev
```

Then replace `assets/screenshot-fixture-vault.svg` with a captured PNG or updated SVG that still uses only fixture data.
