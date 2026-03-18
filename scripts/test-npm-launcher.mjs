import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relPath) {
  return fs.readFileSync(path.join(root, relPath), "utf8");
}

function exists(relPath) {
  return fs.existsSync(path.join(root, relPath));
}

const requiredPaths = [
  "npm/main/package.json",
  "npm/main/bin/rust-silk.js",
  "npm/platforms/darwin-arm64/package.json",
  "npm/platforms/darwin-x64/package.json",
  "npm/platforms/linux-arm64-gnu/package.json",
  "npm/platforms/linux-x64-gnu/package.json",
  "npm/platforms/windows-x64-msvc/package.json",
  "scripts/prepare-npm-release.mjs",
  ".github/workflows/publish-npm.yml",
];

for (const relPath of requiredPaths) {
  assert.ok(exists(relPath), `missing required file: ${relPath}`);
}

const workflow = read(".github/workflows/publish-npm.yml");
assert.match(workflow, /publish-npm:/, "release workflow must define a publish-npm job");
assert.doesNotMatch(workflow, /NPM_TOKEN/, "trusted publishing workflow must not require NPM_TOKEN");
assert.match(workflow, /node-version:\s*22/, "trusted publishing workflow must use Node 22");
assert.match(workflow, /workflow_run:/, "trusted publishing workflow must be triggered by workflow_run");
assert.match(workflow, /workflow_dispatch:/, "trusted publishing workflow must support manual dispatch for recovery publishing");
assert.match(workflow, /workflows:\s*\n\s*-\s*Release/, "trusted publishing workflow must listen to the Release workflow");
assert.match(workflow, /gh release download/, "trusted publishing workflow must download release assets");

const ciWorkflow = read(".github/workflows/ci.yml");
assert.match(ciWorkflow, /actions\/setup-node@v4/, "CI must install Node before npm smoke checks");
assert.match(ciWorkflow, /test-npm-launcher\.mjs/, "CI must run the npm launcher smoke checks");

const readme = read("README.md");
assert.match(readme, /npm install -g rust-silk/, "README must document npm global install");

console.log("npm release smoke checks passed");
