import fs from "node:fs";

// cargo-dist 生成后统一应用项目的发布派发和 Actions 运行时更新。
const file = ".github/workflows/release.yml";
let workflow = fs.readFileSync(file, "utf8")
  .replaceAll("actions/checkout@v4", "actions/checkout@v6")
  .replaceAll("actions/upload-artifact@v4", "actions/upload-artifact@v6")
  .replaceAll("actions/download-artifact@v4", "actions/download-artifact@v6");
if (!workflow.includes("  actions: write")) {
  workflow = workflow.replace('  "contents": "write"', '  "contents": "write"\n  actions: write');
}
if (!workflow.includes("  publish-npm:")) {
  workflow += `
  publish-npm:
    needs: [plan, host]
    if: \${{ always() && needs.host.result == 'success' }}
    runs-on: ubuntu-22.04
    env:
      GH_TOKEN: \${{ secrets.GITHUB_TOKEN }}
      RELEASE_TAG: \${{ needs.plan.outputs.tag }}
    steps:
      - name: Dispatch npm publishing
        run: gh workflow run publish-npm.yml --repo "\$GITHUB_REPOSITORY" --ref "\$RELEASE_TAG" -f "release_tag=\$RELEASE_TAG"
`;
}
fs.writeFileSync(file, workflow);
