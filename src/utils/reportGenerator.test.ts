import test from 'node:test';
import assert from 'node:assert/strict';
import { buildReportData, generateComplianceCsv } from './reportGenerator';
import { EmailMessage, MailboxFolder } from '../types/mail';

const sampleMessages: EmailMessage[] = [
  {
    id: 'msg_001',
    messageId: '<20261101.001@corp.com>',
    folderId: 'inbox',
    subject: 'Quarterly Audit Review Q3',
    from: { name: 'Alice Smith', email: 'alice@corp.com' },
    to: [{ name: 'Bob Jones', email: 'bob@corp.com' }],
    date: '2026-11-01T10:00:00Z',
    sizeBytes: 51200,
    bodyText: 'Please review statements.',
    attachments: [],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'high',
    sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
    md5: '098f6bcd4621d373cade4e832627b4f6',
    batesNumber: 'SEC-TITAN-0001001',
    legalHold: true,
    sourceFormat: 'PST',
  },
];

const sampleFolders: MailboxFolder[] = [
  { id: 'inbox', parentId: null, name: 'Inbox', folderType: 'inbox', unreadCount: 0, totalCount: 1, sizeBytes: 51200, selected: true },
];

test('ReportGenerator: builds compliance metrics correctly', () => {
  const data = buildReportData(sampleMessages, sampleFolders, null, {
    caseNumber: 'CASE-2026-TEST',
  });

  assert.equal(data.caseNumber, 'CASE-2026-TEST');
  assert.equal(data.totalMessages, 1);
  assert.equal(data.totalSizeBytes, 51200);
  assert.equal(data.batesRange.start, 'SEC-TITAN-0001001');
  assert.ok(data.corpusRootSha256.length > 10);
  assert.ok(data.errorLogs.length > 0);
});

test('ReportGenerator: generates well-formed CSV compliance string', () => {
  const data = buildReportData(sampleMessages, sampleFolders, null);
  const csv = generateComplianceCsv(data, sampleMessages);

  assert.ok(csv.includes('# eDiscovery & Migration Compliance Summary'));
  assert.ok(csv.includes('Bates Number,Message-ID,Subject'));
  assert.ok(csv.includes('SEC-TITAN-0001001'));
  assert.ok(csv.includes('9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08'));
  assert.ok(csv.includes('--- Error & Exception Audit Log ---'));
});
