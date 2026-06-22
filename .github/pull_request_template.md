## Summary

What changed?

## Verification

- [ ] `cargo test --workspace`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `npm test --if-present`
- [ ] `npm run build --if-present`
- [ ] `npm run desktop:build`
- [ ] `git diff --check`

## Data Safety

- [ ] No private vault content, client data, personal paths, or private screenshots are included.
- [ ] Runtime indexes, build artifacts, and local app state are not committed.
- [ ] Source files remain canonical; index/cache changes are rebuildable.

## Notes

Anything reviewers should know?
