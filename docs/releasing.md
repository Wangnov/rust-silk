# 发布流程

使用项目固定的 cargo-dist 0.30.3，先提交修复，再准备补丁版本。

```bash
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
node scripts/test-npm-launcher.mjs
bash scripts/prepare-release.sh
cargo release patch --no-publish --no-tag --no-push
cargo release patch --execute --no-publish --no-tag --no-push --no-confirm
git push origin main
gh workflow run release-candidate.yml --ref main
```

候选工作流构建五个平台的正式格式压缩包，并从压缩包运行编解码与 npm 启动器测试。等待候选工作流和同一提交的 CI 全部通过，再执行正式发布：

```bash
cargo release publish --execute --no-confirm
cargo release tag --execute --no-confirm
cargo release push --execute --no-confirm
```

tag 触发 `Release`，成功后显式派发 `Publish npm`；npm 平台包与根包全部发布后，派发 `Post Release Verify` 在五个平台安装并验收。

npm 根包和五个平台包都需要配置 GitHub trusted publisher，仓库为 `Wangnov/rust-silk`，工作流文件为 `publish-npm.yml`。发布环境固定 Node 22 和 npm 11.18.0。

若 npm 上传返回 E404，使用工作流中的 verbose 日志检查 OIDC 交换失败原因，并在 npm 包的 Settings 页面查看绑定状态。`npm trust list` 只显示配置字段，不能据此判断绑定仍有效。页面显示 `Expired` 时，需要删除旧绑定后重新创建；按照 [npm 官方说明](https://docs.npmjs.com/trusted-publishers/)，新绑定必须在两天内成功发布一次才能完成验证。不要为重试 npm 发布另增 crate 版本或改动已发布的 tag。

恢复绑定时，先完成 `npm login --auth-type=web`，再在有 TTY 的终端运行 `npm trust list rust-silk --browser=false` 完成独立的权限验证。该验证页面可选择五分钟内免重复验证，以便批量处理六个包。等待网页验证时不使用 `--json`，避免 npm 缓存授权链接而无法及时显示；完成验证后再使用 JSON 查询配置。

通过 `gh workflow run publish-npm.yml -f release_tag=vX.Y.Z -f dry_run=true` 可验收已有 Release 的组包过程。省略 `dry_run` 可重试发布，已存在的版本会跳过，避免重复发布平台包。

当前候选与安装验收覆盖：`ubuntu-22.04`、`ubuntu-24.04-arm`、`macos-15`、`macos-15-intel`、`windows-latest`。发布前需等待本次候选验收确认 runner 和产物可用。

`release.yml` 由 cargo-dist 生成，通过 `scripts/prepare-release.sh` 重新生成并应用 Actions 版本和发布派发补丁，不直接手工维护生成文件。
