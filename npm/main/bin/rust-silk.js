#!/usr/bin/env node

const { spawn } = require("node:child_process");
const path = require("node:path");

const BINARY_PACKAGES = {
  "darwin-arm64": {
    packageName: "rust-silk-darwin-arm64",
    binaryName: "rust-silk",
  },
  "darwin-x64": {
    packageName: "rust-silk-darwin-x64",
    binaryName: "rust-silk",
  },
  "linux-arm64-gnu": {
    packageName: "rust-silk-linux-arm64-gnu",
    binaryName: "rust-silk",
  },
  "linux-x64-gnu": {
    packageName: "rust-silk-linux-x64-gnu",
    binaryName: "rust-silk",
  },
  "win32-x64-msvc": {
    packageName: "rust-silk-windows-x64-msvc",
    binaryName: "rust-silk.exe",
  },
};

function getLibc() {
  if (process.platform !== "linux") {
    return null;
  }

  const report = process.report?.getReport?.();
  const glibcVersion = report?.header?.glibcVersionRuntime;
  return glibcVersion ? "gnu" : "musl";
}

function getTargetKey() {
  const libc = getLibc();
  if (process.platform === "linux") {
    return `${process.platform}-${process.arch}-${libc}`;
  }
  return `${process.platform}-${process.arch}`;
}

function getBinaryPath() {
  const targetKey = getTargetKey();
  const config = BINARY_PACKAGES[targetKey];

  if (!config) {
    const supported = Object.keys(BINARY_PACKAGES).sort().join(", ");
    console.error(
      `rust-silk does not have a prebuilt npm binary for ${targetKey}. Supported targets: ${supported}`
    );
    process.exit(1);
  }

  const manifestPath = require.resolve(`${config.packageName}/package.json`);
  return path.join(path.dirname(manifestPath), "bin", config.binaryName);
}

const child = spawn(getBinaryPath(), process.argv.slice(2), {
  stdio: "inherit",
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});

child.on("error", (error) => {
  console.error(`failed to start rust-silk binary: ${error.message}`);
  process.exit(1);
});
