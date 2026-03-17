# rust-silk npm Binary Publishing Design

**Context**

`rust-silk` currently uses `cargo-release` tag naming plus `cargo-dist` GitHub Release publishing. The repo does not yet publish installable binaries to npm.

**Goal**

Add npm-based binary distribution without replacing the existing Rust release flow. Users should be able to install the CLI with `npm install -g rust-silk`.

**Decision**

Keep the current tag-driven `cargo-dist` flow as the source of release artifacts, then add an npm distribution layer on top of those artifacts.

**Approaches Considered**

1. Main npm package plus per-platform binary packages
   Recommended. Stable install behavior, no `postinstall` download dependency, and aligns with common binary CLI distribution patterns in npm.

2. Single npm package with `postinstall` downloading from GitHub Releases
   Simpler package layout, but more fragile due to network dependency during install.

3. npm wrapper without binaries
   Lowest maintenance, but poor user experience and not a true npm binary distribution story.

**Chosen Architecture**

- Keep Rust crate versioning and git tags as the source of truth.
- Keep `cargo-dist` building archives for the supported release targets.
- Add an npm workspace under `npm/`:
  - `npm/main`: the public `rust-silk` package
  - `npm/platforms/*`: hidden per-platform binary packages
- Add a small Node launcher in the main package that resolves the installed platform package and executes the real binary.
- Add a release script that:
  - derives the release version from the git tag or environment
  - extracts `cargo-dist` artifacts
  - copies binaries into platform package directories
  - publishes platform packages first, then the main package

**Platform Mapping**

Initial npm distribution should match the existing `cargo-dist` targets:

- `aarch64-apple-darwin` -> `darwin-arm64`
- `x86_64-apple-darwin` -> `darwin-x64`
- `aarch64-unknown-linux-gnu` -> `linux-arm64-gnu`
- `x86_64-unknown-linux-gnu` -> `linux-x64-gnu`
- `x86_64-pc-windows-msvc` -> `windows-x64-msvc`

**Versioning**

- npm package version equals the Rust crate version and git tag version.
- Release automation reads the tag, strips a leading `v`, and applies the same version to every npm package before publishing.

**Publishing Flow**

1. Run `cargo release <level> --execute`
2. Push tag `vX.Y.Z`
3. Existing `cargo-dist` workflow builds and uploads release artifacts
4. New npm publish job downloads those artifacts
5. Script prepares package contents and publishes:
   - all per-platform packages
   - then the main `rust-silk` package

**Error Handling**

- The main launcher prints a clear unsupported-platform error when no package matches the current platform.
- The release script fails fast if an expected target archive or binary is missing.
- Platform packages are marked private to normal workflows except for publish metadata needed for npm.

**Docs Impact**

- README should document npm installation alongside Cargo and cargo-binstall.
- Release notes should continue to point users to GitHub Releases and Cargo; npm becomes an additional install path.
