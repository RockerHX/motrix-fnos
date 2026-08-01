import { MinimalReporter } from 'vitest/node';
import { reportCommandProgress } from './command-progress.mjs';

export default class VitestProgressReporter extends MinimalReporter {
  onTestModuleStart(testModule) {
    reportCommandProgress(`正在加载测试文件：${testModule.relativeModuleId}`);
  }

  onTestCaseReady(testCase) {
    reportCommandProgress(vitestTestProgressDetail(testCase));
    super.onTestCaseReady(testCase);
  }
}

export function vitestTestProgressDetail(testCase) {
  return `正在测试：${testCase.module.relativeModuleId} › ${testCase.fullName}`;
}
