<!--
Thanks for opening a pull request to KDE Connect Browser Extension!
Fill in the sections below. The checklist mirrors CONTRIBUTING.md; please
make sure everything green before pushing.
-->

## Summary

Brief description of the change and why it is needed.

## Type of change

- [ ] Bug fix (non-breaking change fixing an issue)
- [ ] New feature (non-breaking change adding functionality)
- [ ] Refactor / cleanup
- [ ] Documentation
- [ ] Chore (tooling, release, CI)

## Changes

- ...

## Related issues

Closes #123 or "N/A" — write `Closes #<issue number>` to auto-close on merge.

## Testing

Every PR should keep the following green. Check the boxes as you verify:

Host (Rust) — from the repo root:

- [ ] `cargo test`
- [ ] `cargo build` (or `cargo build --release` if the change affects the binary)

Extension (TypeScript) — from `extension/`:

- [ ] `bunx tsc --noEmit`
- [ ] `bunx oxlint --type-aware`
- [ ] `bun test`
- [ ] `bun run build`
- [ ] `bunx web-ext lint --source-dir dist`

Manual verification:

- [ ] Behaviour verified in Firefox/Zen (and Chrome for MV3 paths, if relevant).

## How to verify

Steps a reviewer can follow to reproduce or verify the change, especially
for UI changes.

## Checklist

- [ ] I read [CONTRIBUTING.md](../CONTRIBUTING.md) and followed it.
- [ ] I updated `THIRD_PARTY_NOTICES.md` if dependencies changed.
- [ ] I updated [README.md](../README.md) or [ACCESSIBILITY.md](../ACCESSIBILITY.md)
      if user-facing behaviour changed.
- [ ] My changes are covered by the project's
      [Code of Conduct](../CODE_OF_CONDUCT.md).
