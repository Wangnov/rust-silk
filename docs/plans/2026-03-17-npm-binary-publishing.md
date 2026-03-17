# npm Binary Publishing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add npm binary distribution for `rust-silk` on top of the existing tag-driven `cargo-dist` GitHub Release flow.

**Architecture:** Keep `cargo-dist` as the artifact builder and GitHub Release publisher, then add an npm packaging layer composed of one public main package and multiple platform-specific binary packages. A release preparation script will unpack `cargo-dist` artifacts, inject binaries into the platform package directories, sync package versions, and publish packages in dependency order.

**Tech Stack:** Rust, cargo-release, cargo-dist, GitHub Actions, Node.js, npm

---

### Task 1: Add npm package structure

**Files:**
- Create: `npm/main/package.json`
- Create: `npm/main/bin/rust-silk.js`
- Create: `npm/platforms/darwin-arm64/package.json`
- Create: `npm/platforms/darwin-x64/package.json`
- Create: `npm/platforms/linux-arm64-gnu/package.json`
- Create: `npm/platforms/linux-x64-gnu/package.json`
- Create: `npm/platforms/windows-x64-msvc/package.json`

**Step 1: Write the failing test**

Document expected launcher behavior in a Node smoke test script that resolves the correct platform package path and fails for unsupported targets.

**Step 2: Run test to verify it fails**

Run: `node scripts/test-npm-launcher.mjs`
Expected: FAIL because launcher and package structure do not exist yet.

**Step 3: Write minimal implementation**

Create the npm package manifests and launcher with the minimum fields needed for publish and runtime resolution.

**Step 4: Run test to verify it passes**

Run: `node scripts/test-npm-launcher.mjs`
Expected: PASS

### Task 2: Add release preparation and publish scripts

**Files:**
- Create: `scripts/prepare-npm-release.mjs`
- Create: `scripts/test-npm-launcher.mjs`
- Modify: `package.json` or add root npm metadata if needed

**Step 1: Write the failing test**

Extend the Node smoke test to validate version sync, artifact extraction, binary placement, and generated publish order from fixture archives.

**Step 2: Run test to verify it fails**

Run: `node scripts/test-npm-launcher.mjs`
Expected: FAIL because the release preparation script does not exist yet.

**Step 3: Write minimal implementation**

Implement the script that:
- reads a release version from env or tag
- finds archives in a distribution directory
- extracts each archive
- copies the binary into the matching platform package
- updates all package versions and dependency versions

**Step 4: Run test to verify it passes**

Run: `node scripts/test-npm-launcher.mjs`
Expected: PASS

### Task 3: Wire npm publishing into release workflow

**Files:**
- Modify: `.github/workflows/release.yml`

**Step 1: Write the failing test**

Add workflow-level assertions in the smoke test or a static check that the workflow contains a publish job depending on artifact creation.

**Step 2: Run test to verify it fails**

Run: `node scripts/test-npm-launcher.mjs`
Expected: FAIL because the workflow does not contain npm publish steps.

**Step 3: Write minimal implementation**

Add a `publish-npm` job that:
- waits for the release artifacts
- downloads artifacts
- installs Node
- runs the preparation script
- publishes platform packages first and then the main package

**Step 4: Run test to verify it passes**

Run: `node scripts/test-npm-launcher.mjs`
Expected: PASS

### Task 4: Update user-facing docs

**Files:**
- Modify: `README.md`

**Step 1: Write the failing test**

Extend the smoke test to require npm install instructions in README.

**Step 2: Run test to verify it fails**

Run: `node scripts/test-npm-launcher.mjs`
Expected: FAIL because README does not mention npm.

**Step 3: Write minimal implementation**

Add npm installation instructions in both Chinese and English sections.

**Step 4: Run test to verify it passes**

Run: `node scripts/test-npm-launcher.mjs`
Expected: PASS

### Task 5: Verify end-to-end state

**Files:**
- Verify: `.github/workflows/release.yml`
- Verify: `npm/**`
- Verify: `scripts/**`
- Verify: `README.md`

**Step 1: Run targeted validation**

Run:
- `node scripts/test-npm-launcher.mjs`
- `cargo check`

Expected:
- Node smoke checks pass
- Rust build metadata remains valid

**Step 2: Run broader verification**

Run:
- `cargo test`

Expected:
- Existing Rust tests continue to pass
