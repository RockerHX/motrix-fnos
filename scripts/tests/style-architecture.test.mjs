import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const srcRoot = path.join(repoRoot, "src");

test("Vue 组件样式必须使用同目录 scoped CSS 文件", () => {
  const vueFiles = collectFiles(srcRoot, ".vue");
  let styleFileCount = 0;

  for (const vueFile of vueFiles) {
    const source = readFileSync(vueFile, "utf8");
    const styleTags = [...source.matchAll(/<style\b[^>]*>/g)];

    for (const styleTag of styleTags) {
      const tag = styleTag[0];
      const componentName = path.basename(vueFile, ".vue");
      const expectedSource = `./${componentName}.css`;

      assert.match(tag, /\bscoped\b/, `${vueFile} 的 style 必须保持 scoped`);
      assert.match(
        tag,
        new RegExp(`\\bsrc=["']${escapeRegExp(expectedSource)}["']`),
        `${vueFile} 的 style 必须引用 ${expectedSource}`,
      );
      assert.equal(
        source.slice(styleTag.index + tag.length).match(/^\s*<\/style>/)?.[0],
        "</style>",
        `${vueFile} 不允许把 CSS 内联在 Vue 文件中`,
      );

      const cssFile = path.join(path.dirname(vueFile), `${componentName}.css`);
      assert.ok(existsSync(cssFile), `${vueFile} 引用的 CSS 文件不存在：${cssFile}`);
      styleFileCount += 1;
    }
  }

  assert.equal(styleFileCount, 42, "当前组件样式外置数量应保持为 42 个");

  const mainSource = readFileSync(path.join(srcRoot, "main.ts"), "utf8");
  assert.doesNotMatch(mainSource, /from ["']\.\/features\//, "main.ts 不得引入业务组件样式");
  assert.doesNotMatch(mainSource, /from ["']\.\/layouts\//, "main.ts 不得引入布局组件样式");
});

function collectFiles(directory, extension) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectFiles(entryPath, extension));
    } else if (entry.isFile() && entry.name.endsWith(extension)) {
      files.push(entryPath);
    }
  }
  return files;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
