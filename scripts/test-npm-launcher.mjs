import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

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
assert.match(workflow, /npm install --global npm@11\./, "trusted publishing workflow must install an OIDC-capable npm");
assert.match(workflow, /workflow_dispatch:/, "trusted publishing workflow must support manual dispatch for recovery publishing");
assert.match(read(".github/workflows/release.yml"), /gh workflow run publish-npm\.yml/, "Release must dispatch npm publishing");
assert.match(workflow, /gh workflow run post-release-verify\.yml/, "npm publishing must dispatch installation verification");
assert.match(workflow, /gh release download/, "trusted publishing workflow must download release assets");

const ciWorkflow = read(".github/workflows/ci.yml");
assert.match(ciWorkflow, /actions\/setup-node@v6/, "CI must install Node before npm smoke checks");
assert.match(ciWorkflow, /test-npm-launcher\.mjs/, "CI must run the npm launcher smoke checks");

const readme = read("README.md");
assert.match(readme, /npm install -g rust-silk/, "README must document npm global install");

const launcher = read("npm/main/bin/rust-silk.js");

function launch(platform, arch, glibcVersionRuntime = "2.35") {
  const result = { errors: [], handlers: {}, exits: [], signals: [] };
  const mockRequire = (name) => {
    if (name === "node:path") return path;
    assert.equal(name, "node:child_process");
    return {
      spawn(binary, args, options) {
        result.binary = binary;
        result.args = args;
        result.options = options;
        return {
          on(event, handler) {
            result.handlers[event] = handler;
          },
        };
      },
    };
  };
  mockRequire.resolve = (name) => {
    result.manifest = name;
    return path.join(root, "node_modules", name);
  };
  const exit = new Error("process exited");
  const context = {
    require: mockRequire,
    console: { error: (message) => result.errors.push(message) },
    process: {
      platform,
      arch,
      argv: ["node", "rust-silk", "decode", "-i", "-", "-o", "-"],
      report: { getReport: () => ({ header: { glibcVersionRuntime } }) },
      pid: 1234,
      kill: (pid, signal) => result.signals.push([pid, signal]),
      exit: (code) => {
        result.exits.push(code);
        throw exit;
      },
    },
  };
  try {
    vm.runInNewContext(launcher, context);
  } catch (error) {
    if (error !== exit) throw error;
  }
  result.exit = exit;
  return result;
}

for (const [platform, arch, packageName, binaryName] of [
  ["darwin", "arm64", "rust-silk-darwin-arm64", "rust-silk"],
  ["darwin", "x64", "rust-silk-darwin-x64", "rust-silk"],
  ["linux", "arm64", "rust-silk-linux-arm64-gnu", "rust-silk"],
  ["linux", "x64", "rust-silk-linux-x64-gnu", "rust-silk"],
  ["win32", "x64", "rust-silk-windows-x64-msvc", "rust-silk.exe"],
]) {
  const result = launch(platform, arch);
  assert.equal(result.manifest, `${packageName}/package.json`);
  assert.equal(result.binary, path.join(root, "node_modules", packageName, "bin", binaryName));
  assert.deepEqual(Array.from(result.args), ["decode", "-i", "-", "-o", "-"]);
  assert.equal(result.options.stdio, "inherit");
  assert.equal(result.errors.length, 0);
  assert.throws(() => result.handlers.exit(7, null), (error) => error === result.exit);
  assert.deepEqual(result.exits, [7]);
  result.handlers.exit(null, "SIGTERM");
  assert.deepEqual(result.signals, [[1234, "SIGTERM"]]);
  assert.throws(() => result.handlers.error(new Error("missing binary")), (error) => error === result.exit);
  assert.match(result.errors[0], /failed to start rust-silk binary: missing binary/);
  assert.deepEqual(result.exits, [7, 1]);
}

for (const [platform, arch, glibc, target] of [
  ["linux", "x64", null, "linux-x64-musl"],
  ["win32", "arm64", "2.35", "win32-arm64"],
]) {
  const result = launch(platform, arch, glibc);
  assert.equal(result.binary, undefined);
  assert.deepEqual(result.exits, [1]);
  assert.ok(result.errors[0].includes(`prebuilt npm binary for ${target}.`));
}

console.log("npm launcher and release smoke checks passed");
