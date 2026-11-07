import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createAuditEvent,
  verifyAuditHashChain,
  filterAuditEvents,
  exportAuditLogsCsv,
  INITIAL_AUDIT_LOGS,
} from './auditLogger';

test('AuditLogger: verifies untampered hash chain', () => {
  // Build a 3-event chain
  const e1 = createAuditEvent({
    id: 'AUD-TEST-1',
    timestamp: '2026-11-06T12:00:00Z',
    severity: 'INFO',
    category: 'USER_ACTION',
    action: 'LOGIN',
    actor: 'admin@corp.com',
    targetResource: 'Console',
    description: 'Admin login',
    clientIp: '127.0.0.1',
    processId: 'PID-1',
  }, 'GENESIS');

  const e2 = createAuditEvent({
    id: 'AUD-TEST-2',
    timestamp: '2026-11-06T12:01:00Z',
    severity: 'SECURITY_AUDIT',
    category: 'CRYPTO_HASH',
    action: 'BATES_STAMP',
    actor: 'admin@corp.com',
    targetResource: 'Corpus',
    description: 'Bates stamped',
    clientIp: '127.0.0.1',
    processId: 'PID-1',
  }, e1.eventHash);

  const chain = [e1, e2];
  const res = verifyAuditHashChain(chain);
  assert.equal(res.isValid, true);
  assert.equal(res.totalEventsChecked, 2);

  // Test tamper detection: modify e1 description without recalculating hash
  const tamperedChain = [
    { ...e1, description: 'Tampered malicious injection!' },
    e2,
  ];
  const tamperedRes = verifyAuditHashChain(tamperedChain);
  assert.equal(tamperedRes.isValid, false, 'Tampered chain must be detected as invalid');
});

test('AuditLogger: filters events by category and severity', () => {
  const filtered = filterAuditEvents(INITIAL_AUDIT_LOGS, 'Bates', 'ALL', 'ALL');
  assert.ok(filtered.length >= 1);
  assert.ok(filtered[0].action.includes('BATES'));

  const severityFiltered = filterAuditEvents(INITIAL_AUDIT_LOGS, '', 'ALL', 'SECURITY_AUDIT');
  assert.ok(severityFiltered.every(e => e.severity === 'SECURITY_AUDIT'));
});

test('AuditLogger: generates valid RFC 3161 CSV format', () => {
  const csv = exportAuditLogsCsv(INITIAL_AUDIT_LOGS);
  assert.ok(csv.includes('# Mail2Mail Studio - Forensic & Security Audit Log Stream'));
  assert.ok(csv.includes('Event ID,Timestamp UTC,Timestamp Nanos'));
  assert.ok(csv.includes('AUD-2026-0001'));
});
