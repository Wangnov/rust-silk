import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const rootDir = process.cwd();
const distDir = path.resolve(process.env.DIST_ARTIFACT_DIR ?? "artifacts");
const explicitVersion = process.env.RELEASE_VERSION;

const TARGETS = [
  {
    distTarget: "aarch64-apple-darwin",
    packageDir: "npm/platforms/darwin-arm64",
    packageName: "rust-silk-darwin-arm64",
    binaryName: "rust-silk",
  },
  {
    distTarget: "x86_64-apple-darwin",
    packageDir: "npm/platforms/darwin-x64",
    packageName: "rust-silk-darwin-x64",
    binaryName: "rust-silk",
  },
  {
    distTarget: "aarch64-unknown-linux-gnu",
    packageDir: "npm/platforms/linux-arm64-gnu",
    packageName: "rust-silk-linux-arm64-gnu",
    binaryName: "rust-silk",
  },
  {
    distTarget: "x86_64-unknown-linux-gnu",
    packageDir: "npm/platforms/linux-x64-gnu",
    packageName: "rust-silk-linux-x64-gnu",
    binaryName: "rust-silk",
  },
  {
    distTarget: "x86_64-pc-windows-msvc",
    packageDir: "npm/platforms/windows-x64-msvc",
    packageName: "rust-silk-windows-x64-msvc",
    binaryName: "rust-silk.exe",
  },
];

function fail(message) {
  throw new Error(message);
}

function readJson(relPath) {
  return JSON.parse(fs.readFileSync(path.join(rootDir, relPath), "utf8"));
}

function writeJson(relPath, value) {
  fs.writeFileSync(
    path.join(rootDir, relPath),
    `${JSON.stringify(value, null, 2)}\n`
  );
}

function resolveVersion() {
  if (explicitVersion) {
    return explicitVersion.replace(/^v/, "");
  }

  const git = spawnSync("git", ["describe", "--tags", "--exact-match"], {
    cwd: rootDir,
    encoding: "utf8",
  });

  if (git.status !== 0) {
    fail(
      "unable to determine release version from RELEASE_VERSION or current git tag"
    );
  }

  return git.stdout.trim().replace(/^v/, "");
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function listArtifactFiles() {
  if (!fs.existsSync(distDir)) {
    fail(`artifact directory not found: ${distDir}`);
  }

  return walk(distDir);
}

function findArchive(distTarget, files) {
  const archive = files.find((file) => {
    const base = path.basename(file);
    return (
      base.includes(distTarget) &&
      (base.endsWith(".tar.gz") || base.endsWith(".zip"))
    );
  });

  if (!archive) {
    fail(`missing release archive for target ${distTarget} in ${distDir}`);
  }

  return archive;
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: rootDir,
    encoding: "utf8",
  });

  if (result.status !== 0) {
    const stderr = result.stderr?.trim();
    const stdout = result.stdout?.trim();
    fail(
      `${command} ${args.join(" ")} failed` +
        (stderr ? `: ${stderr}` : stdout ? `: ${stdout}` : "")
    );
  }
}

function extractArchive(archivePath, outputDir) {
  ensureDir(outputDir);
  if (archivePath.endsWith(".zip")) {
    run("unzip", ["-q", archivePath, "-d", outputDir]);
    return;
  }

  run("tar", ["-xzf", archivePath, "-C", outputDir]);
}

function walk(dirPath) {
  const entries = fs.readdirSync(dirPath, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      files.push(...walk(fullPath));
    } else {
      files.push(fullPath);
    }
  }

  return files;
}

function copyBinary(target, files) {
  const archive = findArchive(target.distTarget, files);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "rust-silk-npm-"));

  extractArchive(archive, tempDir);

  const candidates = walk(tempDir).filter((file) => path.basename(file) === target.binaryName);
  if (candidates.length === 0) {
    fail(`binary ${target.binaryName} not found in archive ${path.basename(archive)}`);
  }

  const destinationDir = path.join(rootDir, target.packageDir, "bin");
  ensureDir(destinationDir);
  const destination = path.join(destinationDir, target.binaryName);
  fs.copyFileSync(candidates[0], destination);
  fs.chmodSync(destination, 0o755);
}

function syncVersions(version) {
  const mainPath = "npm/main/package.json";
  const main = readJson(mainPath);
  main.version = version;

  for (const target of TARGETS) {
    main.optionalDependencies[target.packageName] = version;
    const pkgPath = `${target.packageDir}/package.json`;
    const pkg = readJson(pkgPath);
    pkg.version = version;
    writeJson(pkgPath, pkg);
  }

  writeJson(mainPath, main);
}

function main() {
  const version = resolveVersion();
  const files = listArtifactFiles();

  syncVersions(version);

  for (const target of TARGETS) {
    copyBinary(target, files);
  }

  console.log(`prepared npm release packages for version ${version}`);
}

main();
