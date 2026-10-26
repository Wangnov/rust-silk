#!/usr/bin/env bash
set -euo pipefail
node --input-type=module <<'JS'
import fs from "node:fs";
import { spawnSync } from "node:child_process";
const file = "dist-workspace.toml";
const config = fs.readFileSync(file, "utf8");
try {
  // 生成原始 CI 时临时移除跳过生成的配置，随后恢复并应用项目补丁。
  fs.writeFileSync(file, config.replace(/^allow-dirty = \["ci"\]\n/m, ""));
  const result = spawnSync("dist", ["generate", "--mode", "ci"], { stdio: "inherit" });
  if (result.status !== 0) throw new Error("cargo-dist CI generation failed");
} finally {
  fs.writeFileSync(file, config);
}
JS
node scripts/patch-release-workflow.mjs
