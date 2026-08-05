import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '../..');
const rustSourceRoot = path.join(repoRoot, 'server/src');
const frontendSourceRoot = path.join(repoRoot, 'src');

test('Rust 测试实现与测试专用接口不进入业务模块', () => {
  for (const file of collectFiles(rustSourceRoot, ['.rs'])) {
    if (isDedicatedRustTestFile(file)) continue;

    const source = readFileSync(file, 'utf8');
    const relativePath = path.relative(repoRoot, file);
    assert.doesNotMatch(
      source,
      /^\s*#\[(?:test|tokio::test|async_std::test)/m,
      `业务模块不得包含测试函数：${relativePath}`,
    );

    for (const declaration of testDeclarations(source)) {
      const allowedModule = declaration === 'mod tests;';
      const allowedSharedSupport =
        relativePath === 'server/src/lib.rs' && declaration === 'pub(crate) mod test_support;';
      assert.ok(
        allowedModule || allowedSharedSupport,
        `业务模块只允许外部测试模块声明，发现测试专用声明：${relativePath} -> ${declaration}`,
      );
    }
  }
});

test('前端测试依赖只存在于独立测试文件或测试支撑目录', () => {
  for (const file of collectFiles(frontendSourceRoot, ['.ts', '.tsx', '.vue'])) {
    if (isDedicatedFrontendTestFile(file) || isFrontendTestSupport(file)) continue;

    const source = readFileSync(file, 'utf8');
    assert.doesNotMatch(
      source,
      /(?:from\s+['"](?:vitest|@vue\/test-utils)['"]|require\(\s*['"](?:vitest|@vue\/test-utils)['"]\s*\))/,
      `业务模块不得直接依赖测试工具：${path.relative(repoRoot, file)}`,
    );
  }
});

function testDeclarations(source) {
  return [...source.matchAll(/#\[cfg\(test\)\]\s*\r?\n\s*([^\r\n]+)/g)].map((match) => match[1].trim());
}

function isDedicatedRustTestFile(file) {
  const relativePath = path.relative(rustSourceRoot, file);
  return path.basename(file) === 'tests.rs' || relativePath.split(path.sep).includes('tests');
}

function isDedicatedFrontendTestFile(file) {
  return /\.spec\.(?:ts|tsx|vue)$/.test(file);
}

function isFrontendTestSupport(file) {
  return path.relative(frontendSourceRoot, file).split(path.sep).includes('test');
}

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
