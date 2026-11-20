# Contributing

Thank you for your interest in contributing to
[KDE Connect Browser Extension](../README.md)!

This project has two components that must both stay green:

- **`src/` + `Cargo.toml`** — the native messaging host (Rust, `zbus`, tokio).
- **`extension/`** — the Firefox/Chromium extension (TypeScript, tsdown, Bun).

## Reporting bugs

- **Extension bugs** — open a GitHub issue with the browser, version
  (see `extension/static/manifest.json`), and steps to reproduce. Please
  include the `--version` output of the installed host if relevant.
- **Native host bugs** — include output of `kdeconnect-chrome-extension --devices`.
- **Security** — see [SECURITY.md](SECURITY.md).

## Development setup

### Host (Rust)

```bash
cargo build --release          # or cargo build for local iteration
```

Install/reinstall the host:

```bash
./target/release/kdeconnect-chrome-extension --install
```

### Extension

```bash
cd extension && bun install
```

Build the extension:

```bash
bun run build
```

### Regenerating types

`cargo test` runs `ts-rs` and regenerates `extension/src/generated/*`.
If you change `src/types.rs`, re-run `cargo test` and commit the changes.

## Running the tests

```bash
cargo test                     # host unit tests + type regeneration
cargo build                    # host build

cd extension
bunx tsc --noEmit              # extension typecheck
bunx oxlint --type-aware       # extension lint
bun test                       # extension tests (reconnect, menus, icons, …)
bun run build                  # extension bundle
bunx web-ext lint --source-dir dist   # extension bundle audit (manifest/locale checks)
```

Every PR must keep the full suite above green. Prefer running
`cargo test && cargo build` and the extension suite before pushing.

## Code style

- **Rust** — `cargo fmt` and `cargo clippy` on changes; follow the existing
  error-wrapping style (`anyhow::Context`) and docs on `pub` items.
- **TypeScript** — `oxlint` clean, `bunx tsc --noEmit` clean, formatted with
  `bun run fmt` (oxfmt) in `extension/`. Add tests for new logic: the popup,
  menus, icons and reconnect policy all have `*.test.ts` suites.
- **Commits** — conventional style like `feat(extension): …`,
  `fix(host): …`, `docs: …`, `chore: …`.

## Pull requests

- Branch from the repo's current default branch and keep the change scoped
  to one concern.
- Explain *why* in the PR description; link any issue you fix.
- Fill in the PR template ([`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md)):
  leave every checkbox ticked, and in "How to verify" describe how a reviewer
  can reproduce or verify the change.
- If a change touches user-facing UI, mention how you verified it (browser
  manual checks count).
- Update `THIRD_PARTY_NOTICES.md` when you add new dependencies.

## Docs

When adding or changing user-facing behavior:

- Update [README.md](README.md) (installation, usage, building).
- Update [ACCESSIBILITY.md](ACCESSIBILITY.md) if it affects the UI
  (popup contents, ARIA labels, colour choices, etc.).
- Mention it in the PR.

For the project's values, see the [Code of Conduct](CODE_OF_CONDUCT.md).
