#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { lstatSync, readdirSync, readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import process from "node:process";

const repositoryRoot = resolve(new URL("../..", import.meta.url).pathname);

// Long-lived documents require an explicit allowlist entry. The extension plan
// remains grandfathered only until ME-07/ME-08 is closed.
export const ALLOWED_DOCUMENTS = new Set([
  "docs/api-contract.md",
  "docs/architecture.md",
  "docs/design/DESIGN.md",
  "docs/design/ui-product-requirements.md",
  "docs/development-scripts.md",
  "docs/fpk-packaging.md",
  "docs/future-development-plan.md",
  "docs/motrix-extension-manual-acceptance.md",
  "docs/motrix-extension-support-development-plan.md",
]);

const LOCAL_ONLY_PREFIX = "docs/verification/";
const REMOVED_DOCUMENT_REFERENCES = [
  "fnos-open-api-development-plan.md",
  "security-access-development-plan.md",
  "interaction-motion-plan.md",
  "jsonrpc-remote-access.md",
];

export function checkDocumentPolicy(root = repositoryRoot) {
  const violations = [];
  const docsRoot = join(root, "docs");

  for (const file of walkFiles(docsRoot)) {
    const relativePath = toRepositoryPath(root, file);
    if (isIgnoredByGit(root, relativePath)) {
      continue;
    }
    if (relativePath.startsWith(LOCAL_ONLY_PREFIX)) {
      continue;
    }
    if (lstatSync(file).isSymbolicLink()) {
      violations.push(`${relativePath}: docs 文档不得通过符号链接引入`);
      continue;
    }
    if (!ALLOWED_DOCUMENTS.has(relativePath)) {
      violations.push(`${relativePath}: 不在长期文档白名单中`);
      continue;
    }

    const contents = readFileSync(file, "utf8");
    for (const removedReference of REMOVED_DOCUMENT_REFERENCES) {
      if (contents.includes(removedReference)) {
        violations.push(`${relativePath}: 引用了已删除的过渡文档 ${removedReference}`);
      }
    }
  }

  for (const trackedPath of trackedVerificationFiles(root)) {
    violations.push(`${trackedPath}: docs/verification 只允许本地存在，不得提交`);
  }

  const futurePlan = join(root, "docs/future-development-plan.md");
  if (readFileSync(futurePlan, "utf8").includes("本文件是当前唯一的开发计划来源") === false) {
    violations.push("docs/future-development-plan.md: 必须声明其为唯一开发计划来源");
  }

  return violations;
}

function walkFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      files.push(file);
    } else if (entry.isDirectory()) {
      files.push(...walkFiles(file));
    } else {
      files.push(file);
    }
  }
  return files;
}

function trackedVerificationFiles(root) {
  try {
    const output = execFileSync("git", ["ls-files", "-z", "--", "docs/verification"], {
      cwd: root,
      encoding: "utf8",
    });
    return output.split("\0").filter(Boolean);
  } catch {
    return [];
  }
}

function isIgnoredByGit(root, relativePath) {
  try {
    execFileSync("git", ["check-ignore", "-q", "--", relativePath], { cwd: root });
    return true;
  } catch {
    return false;
  }
}

function toRepositoryPath(root, file) {
  return relative(root, file).split("\\").join("/");
}

if (process.argv[1] === new URL(import.meta.url).pathname) {
  const violations = checkDocumentPolicy();
  if (violations.length > 0) {
    console.error("文档生命周期守卫失败：");
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exitCode = 1;
  } else {
    console.log("文档生命周期守卫通过：docs 仅包含长期白名单文档。");
  }
}
