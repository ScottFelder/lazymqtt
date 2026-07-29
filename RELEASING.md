# Releasing

lazymqtt uses two GitHub Actions workflows:

- **`.github/workflows/ci.yml`** — runs on every push/PR to `master`: formatting
  (`cargo fmt --check`), `clippy -D warnings`, and `cargo test` + release build
  on Linux and macOS.
- **`.github/workflows/release.yml`** — runs when a `v*.*.*` tag is pushed. It:
  1. builds release binaries for `aarch64-apple-darwin`, `x86_64-apple-darwin`,
     and `x86_64-unknown-linux-gnu`, and attaches them (as `.tar.gz`) to the
     GitHub Release for the tag;
  2. publishes the crate to crates.io (`cargo publish`);
  3. regenerates the formula to point at the new binaries and pushes it to the
     dedicated Homebrew tap repo, [`ScottFelder/homebrew-lazymqtt`](https://github.com/ScottFelder/homebrew-lazymqtt).

## Versioning

Semantic versioning, incrementing the patch number for ordinary releases:
`0.1.0 → 0.1.1 → 0.1.2 → …`. The version lives in `Cargo.toml`, is baked into
the binary (`--version` / the status bar via `CARGO_PKG_VERSION`), and the tag
must match it (`v0.1.1` ⇢ `version = "0.1.1"`).

## Cutting a release

1. Move the accumulated notes in `CHANGELOG.md` from `## [Unreleased]` into a new
   `## [X.Y.Z] - YYYY-MM-DD` section, and add the matching link references at the
   bottom of the file.
2. Bump `version` in `Cargo.toml` to `X.Y.Z`.
3. Commit both: `git commit -am "Release vX.Y.Z"`.
4. Tag and push:
   ```bash
   git tag vX.Y.Z
   git push origin master --tags
   ```
5. The Release workflow does the rest. Watch it under the repo's **Actions** tab.

## One-time setup (required before the first release)

- **crates.io token** — create an API token at
  <https://crates.io/settings/tokens> (scope: publish-new + publish-update), then
  add it as a repository secret named **`CARGO_REGISTRY_TOKEN`**
  (Settings → Secrets and variables → Actions). The crate name `lazymqtt` must be
  available/owned by you; the first `cargo publish` claims it.
- **Homebrew tap** — create a separate repo named **`homebrew-lazymqtt`** (the
  `homebrew-` prefix is what lets `brew tap scottfelder/lazymqtt` resolve to it).
  It holds only `Formula/lazymqtt.rb`, which the release workflow keeps updated.
  Then create a **`HOMEBREW_TAP_TOKEN`** repository secret on *this* repo: a
  fine-grained PAT with **Contents: read and write** on the `homebrew-lazymqtt`
  repo (the workflow uses it to push the regenerated formula there).
  Users install with:
  ```bash
  brew tap scottfelder/lazymqtt
  brew trust scottfelder/lazymqtt   # required for any third-party tap
  brew install lazymqtt
  ```
  The `brew trust` step is a Homebrew-wide requirement for third-party taps (it
  refuses to load an untrusted tap's formula), unavoidable short of getting into
  `homebrew-core`.

## Notes

- The Homebrew formula installs a **prebuilt binary** (no compile on the user's
  machine). Its `brew test` runs `lazymqtt --version`.
- Re-running a release for a version already on crates.io will fail the
  `crates` job (crates.io versions are immutable) — bump the patch instead.
