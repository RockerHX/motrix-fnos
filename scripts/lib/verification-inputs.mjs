export const verificationGroups = ['scripts', 'rust', 'frontend'];

export function classifyVerificationFile(file) {
  if (file.startsWith('scripts/tests/')) return 'scripts-test';
  if (file.startsWith('scripts/extended/')) return 'aria2-test';
  if (file.startsWith('e2e/')) return 'browser-test';
  if (/^scripts\/verify\/test-fnos-[^/]+\.sh$/.test(file)) return 'shell-test';
  if (file.startsWith('src/test/') || /^src\/.*\.spec\.(?:ts|tsx|vue)$/.test(file)) return 'frontend-test';
  if (file.startsWith('src/') || file.startsWith('public/') || file === 'CHANGELOG.md'
    || file === 'index.html' || /^(?:vite|vitest|uno)\.config\.[cm]?[jt]s$/.test(file)
    || /^tsconfig(?:\.[^/]+)?\.json$/.test(file)) return 'frontend';
  if (file === 'server/tests/runtime_stability.rs' || file.startsWith('server/tests/runtime_stability/')) return 'stability-test';
  if (file.startsWith('server/tests/') || file === 'server/src/test_support.rs'
    || /^server\/.*\/tests(?:\/|\.rs$)/.test(file)) return 'rust-test';
  if (file.startsWith('server/') || file.startsWith('.cargo/') || file === 'rust-toolchain.toml') return 'rust';
  // Rust 安装与密码重置集成测试会直接执行这三个 Shell 文件。
  if (/^packaging\/fnos\/cmd\/(?:common\.sh|install_callback|reset-web-auth)$/.test(file)) return 'packaging-rust';
  if (file.startsWith('packaging/fnos/')) return 'packaging';
  if (file.startsWith('assets/aria2/')) return 'aria2';
  if (file.startsWith('.github/') || file.startsWith('.githooks/')) return 'scripts';
  if (file.startsWith('scripts/')) return 'shared';
  if (file === 'docs/maintenance.md' || file === 'docs/api-contract.md') return 'checked-docs';
  const documentation = file.startsWith('docs/') || /(?:^|\/)(?:README|AGENTS|SECURITY|LICENSE|NOTICE)\./i.test(file);
  if (/(?:^|\/)(?:LICENSE|NOTICE)$/.test(file) || (documentation
    && /\.(?:md|mdx|txt|rst|adoc|png|jpg|jpeg|gif|webp|avif|ico|bmp|tif|tiff|svg|pdf|woff2?|ttf|otf|mp3|wav|mp4|webm)$/i.test(file))) return 'docs';
  return 'shared';
}

export function verificationGroupsForFile(file) {
  switch (classifyVerificationFile(file)) {
    case 'docs': case 'checked-docs': return [];
    case 'frontend': case 'frontend-test': return ['scripts', 'frontend'];
    case 'rust': case 'rust-test': case 'stability-test': case 'packaging-rust': case 'aria2': return ['scripts', 'rust'];
    case 'scripts': case 'scripts-test': case 'shell-test': case 'packaging': case 'aria2-test': case 'browser-test': return ['scripts'];
    default: return verificationGroups;
  }
}
