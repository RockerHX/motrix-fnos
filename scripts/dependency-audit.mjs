#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import process from 'node:process';

export const BLOCKING_SEVERITIES = new Set(['high', 'critical']);

export function evaluateAuditReports(reports) {
  const unavailable = reports.filter((report) => report.status === 'unavailable');
  if (unavailable.length > 0) {
    return {
      status: 'error',
      message: unavailable.map((report) => `${report.tool}: ${report.message}`).join('\n'),
      findings: [],
    };
  }

  const findings = reports.flatMap((report) => report.findings ?? []);
  const blocking = findings.filter((finding) => BLOCKING_SEVERITIES.has(finding.severity));
  return {
    status: blocking.length > 0 ? 'fail' : 'pass',
    message: blocking.length > 0 ? `${blocking.length} 个高危或严重依赖漏洞阻断发布` : '未发现高危或严重依赖漏洞',
    findings,
  };
}

export function parsePnpmAuditReport(report) {
  const vulnerabilities = report?.metadata?.vulnerabilities;
  if (!vulnerabilities || typeof vulnerabilities !== 'object') {
    return [];
  }

  return Object.entries(vulnerabilities).flatMap(([severity, count]) => {
    if (!Number.isInteger(count) || count <= 0) return [];
    return Array.from({ length: count }, () => ({ tool: 'pnpm', severity }));
  });
}

export function parseCargoAuditReport(report) {
  const vulnerabilities = report?.vulnerabilities?.list;
  if (!Array.isArray(vulnerabilities)) {
    return [];
  }

  return vulnerabilities.map((entry) => ({
    tool: 'cargo-audit',
    severity: normalizeSeverity(entry?.advisory?.severity ?? entry?.severity),
    id: entry?.advisory?.id,
    package: entry?.package?.name,
  }));
}

export function normalizeSeverity(value) {
  const normalized = String(value ?? '').trim().toLowerCase();
  return normalized === 'moderate' ? 'moderate' : normalized || 'unknown';
}

export function runAuditCommand(command, args, spawn = spawnSync) {
  const result = spawn(command, args, {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (result.error || result.status === 127) {
    return {
      status: 'unavailable',
      message: result.error?.message ?? `${command} 未找到`,
    };
  }

  let report;
  try {
    report = JSON.parse(result.stdout || '');
  } catch {
    return {
      status: 'unavailable',
      message: `${command} 未返回可解析的 JSON（退出码 ${result.status ?? 'unknown'}）`,
    };
  }

  return {
    status: 'ok',
    findings: command === 'cargo' ? parseCargoAuditReport(report) : parsePnpmAuditReport(report),
  };
}

function main() {
  const reports = [
    runNamedAudit('cargo-audit', 'cargo', ['audit', '--json', '--file', 'server/Cargo.lock']),
    runNamedAudit('pnpm', 'pnpm', ['audit', '--prod', '--json', '--audit-level', 'low']),
  ];
  const result = evaluateAuditReports(reports);

  for (const finding of result.findings) {
    const detail = [finding.tool, finding.severity, finding.id, finding.package].filter(Boolean).join(' ');
    if (BLOCKING_SEVERITIES.has(finding.severity)) {
      console.error(`阻断发布：${detail}`);
    } else if (finding.severity !== 'unknown') {
      console.warn(`依赖审计提示：${detail}`);
    }
  }
  if (result.status === 'error') {
    console.error(`依赖审计失败：${result.message}`);
    process.exitCode = 2;
  } else if (result.status === 'fail') {
    console.error(result.message);
    process.exitCode = 1;
  } else {
    console.log(result.message);
  }
}

function runNamedAudit(tool, command, args) {
  return { tool, ...runAuditCommand(command, args) };
}

if (process.argv[1]?.endsWith('dependency-audit.mjs')) {
  main();
}
