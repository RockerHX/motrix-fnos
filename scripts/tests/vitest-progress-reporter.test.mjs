import assert from 'node:assert/strict';
import test from 'node:test';
import { vitestTestProgressDetail } from '../verify/vitest-progress-reporter.mjs';

test('Vitest 当前任务包含测试文件和完整用例名', () => {
  assert.equal(
    vitestTestProgressDetail({
      module: { relativeModuleId: 'src/components/EngineStatusPanel.spec.ts' },
      fullName: 'EngineStatusPanel > 显示引擎状态',
    }),
    '正在测试：src/components/EngineStatusPanel.spec.ts › EngineStatusPanel > 显示引擎状态',
  );
});
