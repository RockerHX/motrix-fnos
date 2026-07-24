import assert from 'node:assert/strict';
import test from 'node:test';
import {
  evaluateAuditReports,
  parseCargoAuditReport,
  parsePnpmAuditReport,
  runAuditCommand,
} from '../dependency-audit.mjs';

test('无漏洞审计结果通过', () => {
  const result = evaluateAuditReports([
    { tool: 'cargo-audit', status: 'ok', findings: parseCargoAuditReport({ vulnerabilities: { list: [] } }) },
    {
      tool: 'pnpm',
      status: 'ok',
      findings: parsePnpmAuditReport({ metadata: { vulnerabilities: { low: 0, moderate: 0, high: 0, critical: 0 } } }),
    },
  ]);

  assert.equal(result.status, 'pass');
});

test('高危漏洞阻断审计', () => {
  const result = evaluateAuditReports([
    {
      tool: 'pnpm',
      status: 'ok',
      findings: parsePnpmAuditReport({ metadata: { vulnerabilities: { low: 1, moderate: 0, high: 1, critical: 0 } } }),
    },
  ]);

  assert.equal(result.status, 'fail');
  assert.equal(result.findings.length, 2);
});

test('审计工具不可用时失败', () => {
  const result = evaluateAuditReports([{ tool: 'cargo-audit', status: 'unavailable', message: 'command not found' }]);

  assert.equal(result.status, 'error');
});

test('审计命令退出非零但返回 JSON 时仍解析漏洞结果', () => {
  const result = runAuditCommand('pnpm', ['audit'], () => ({
    status: 1,
    stdout: JSON.stringify({ metadata: { vulnerabilities: { low: 0, moderate: 0, high: 1, critical: 0 } } }),
    stderr: '',
  }));

  assert.equal(result.status, 'ok');
  assert.equal(result.findings[0].severity, 'high');
});
