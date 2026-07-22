import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const srcRoot = path.join(repoRoot, "src");
const tokensPath = path.join(srcRoot, "styles/tokens.css");
const naiveProviderPath = path.join(srcRoot, "app/providers/NaiveProvider.vue");

const expectedTokens = {
  "--app-color-brand-navy": "#1e3a5f",
  "--app-color-brand-deep": "#102846",
  "--app-color-brand-blue": "#3374db",
  "--app-color-brand-blue-hover": "#5da9ff",
  "--app-color-brand-blue-pressed": "#285bae",
  "--app-color-brand-blue-suppl": "#a8c8f0",
  "--app-color-brand-selected": "#1e3a5f",
  "--app-color-on-brand": "#f4f8fd",
  "--app-text-accent": "#3374db",
  "--app-text-accent-soft": "#5da9ff",
};

const primaryOverrides = {
  primaryColor: "#3374db",
  primaryColorHover: "#5da9ff",
  primaryColorPressed: "#285bae",
  primaryColorSuppl: "#a8c8f0",
};

const legacyBrandGreens = [
  "#68ae5a",
  "#7bc96d",
  "#57964b",
  "#8ef08a",
  "#66e39a",
  "#67dca0",
];

test("logo 蓝色主题令牌与 Naive UI 主色保持一致", () => {
  const tokenSource = readFileSync(tokensPath, "utf8");
  const providerSource = readFileSync(naiveProviderPath, "utf8");

  for (const [name, expectedValue] of Object.entries(expectedTokens)) {
    const match = tokenSource.match(new RegExp(`${escapeRegExp(name)}\\s*:\\s*([^;]+);`));
    assert.ok(match, `缺少主题令牌：${name}`);
    assert.equal(match[1].trim().toLowerCase(), expectedValue, `${name} 的值不符合 logo 蓝色规范`);
  }

  for (const [field, expectedValue] of Object.entries(primaryOverrides)) {
    const match = providerSource.match(new RegExp(`${escapeRegExp(field)}\\s*:\\s*["']([^"']+)["']`));
    assert.ok(match, `Naive UI 缺少主色覆盖：${field}`);
    assert.equal(match[1].toLowerCase(), expectedValue, `${field} 未与蓝色主题令牌保持一致`);
  }
});

test("运行时代码不再包含旧绿色品牌色", () => {
  const runtimeFiles = [path.join(repoRoot, "index.html"), ...collectFiles(srcRoot, [".css", ".ts", ".vue"])];
  const legacyPattern = new RegExp(legacyBrandGreens.join("|"), "i");

  for (const file of runtimeFiles) {
    const source = readFileSync(file, "utf8");
    assert.doesNotMatch(source, legacyPattern, `运行时代码仍包含旧绿色品牌色：${file}`);
  }
});

test("业务组件不得散落硬编码品牌蓝色", () => {
  const brandHexPattern = /#(?:1e3a5f|102846|3374db|5da9ff|285bae|a8c8f0|f4f8fd)\b/i;
  const componentFiles = collectFiles(srcRoot, [".css", ".vue"]).filter((file) => {
    return file !== tokensPath && file !== naiveProviderPath;
  });

  for (const file of componentFiles) {
    const source = readFileSync(file, "utf8");
    assert.doesNotMatch(source, brandHexPattern, `组件必须通过主题 token 使用品牌色：${file}`);
  }
});

function collectFiles(directory, extensions) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectFiles(entryPath, extensions));
    } else if (entry.isFile() && extensions.some((extension) => entry.name.endsWith(extension))) {
      files.push(entryPath);
    }
  }
  return files;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&");
}
