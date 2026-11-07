import test from 'node:test';
import assert from 'node:assert/strict';
import { matchesFilter, filterMessages } from './filterEngine';
import { splitMessagesIntoVolumes } from './pstSplitter';
import { calculateDeltaDiff, createUpdatedCheckpoint } from './deltaSync';
import { simpleHash, applyBatesStamping, deduplicateMessages, generateForensicManifest } from './forensics';
import { buildDirectPst } from './pstGenerator';
import { EmailMessage, MailboxFolder } from '../types/mail';

const sampleMessages: EmailMessage[] = [
  {
    id: 'msg_001',
    messageId: '<20261101.001@corp.com>',
    conversationId: 'conv_101',
    folderId: 'inbox',
    subject: 'Quarterly Audit Review Q3',
    from: { name: 'Alice Smith', email: 'alice@corp.com' },
    to: [{ name: 'Bob Jones', email: 'bob@corp.com' }],
    date: '2026-11-01T10:00:00Z',
    sizeBytes: 1024 * 50, // 50 KB
    bodyText: 'Please review the attached quarterly statements.',
    attachments: [
      {
        id: 'att_1',
        filename: 'audit_q3.xlsx',
        sizeBytes: 1024 * 30,
        mimeType: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
        sha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
      },
    ],
    mapiProperties: [
      { tag: '0x0037001F', name: 'PR_SUBJECT_W', type: 'PT_UNICODE', value: 'Quarterly Audit Review Q3' },
    ],
    isRead: true,
    isFlagged: false,
    importance: 'high',
    sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
    md5: '098f6bcd4621d373cade4e832627b4f6',
    sourceFormat: 'PST',
  },
  {
    id: 'msg_002',
    messageId: '<20261102.002@corp.com>',
    conversationId: 'conv_101', // Same thread as msg_001
    folderId: 'inbox',
    subject: 'Re: Quarterly Audit Review Q3',
    from: { name: 'Bob Jones', email: 'bob@corp.com' },
    to: [{ name: 'Alice Smith', email: 'alice@corp.com' }],
    date: '2026-11-02T14:30:00Z',
    sizeBytes: 1024 * 20, // 20 KB
    bodyText: 'Looks good to me, proceeding with filing.',
    attachments: [],
    mapiProperties: [],
    isRead: false,
    isFlagged: true,
    importance: 'normal',
    sha256: '5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8',
    md5: '5d41402abc4b2a76b9719d911017c592',
    sourceFormat: 'PST',
  },
  {
    id: 'msg_003',
    messageId: '<20261105.003@external.org>',
    conversationId: 'conv_102',
    folderId: 'sent',
    subject: 'Vendor Contract Execution Notice',
    from: { name: 'Legal Dept', email: 'legal@corp.com' },
    to: [{ name: 'Vendor Rep', email: 'contact@external.org' }],
    date: '2026-11-05T09:15:00Z',
    sizeBytes: 1024 * 150, // 150 KB
    bodyText: 'Agreement has been signed and finalized.',
    attachments: [],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'high',
    sha256: '4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a',
    md5: '7d793037a0760186574b0282f2f435e7',
    sourceFormat: 'MBOX',
  },
];

const sampleFolders: MailboxFolder[] = [
  { id: 'inbox', parentId: null, name: 'Inbox', folderType: 'inbox', unreadCount: 1, totalCount: 2, sizeBytes: 70 * 1024 },
  { id: 'sent', parentId: null, name: 'Sent Items', folderType: 'sent', unreadCount: 0, totalCount: 1, sizeBytes: 150 * 1024 },
];

test('FilterEngine: sender, date, and attachment filtering', () => {
  // Test sender search
  const aliceMatches = filterMessages(sampleMessages, { senderSearch: 'alice' });
  assert.equal(aliceMatches.length, 1);
  assert.equal(aliceMatches[0].id, 'msg_001');

  // Test attachment filter
  const attMatches = filterMessages(sampleMessages, { hasAttachments: true });
  assert.equal(attMatches.length, 1);
  assert.equal(attMatches[0].subject, 'Quarterly Audit Review Q3');

  // Test date filtering
  const dateMatches = filterMessages(sampleMessages, {
    startDate: '2026-11-02',
    endDate: '2026-11-06',
  });
  assert.equal(dateMatches.length, 2);
  assert.ok(dateMatches.some(m => m.id === 'msg_002'));
  assert.ok(dateMatches.some(m => m.id === 'msg_003'));
});

test('PstSplitter: volume sizing and thread preservation', () => {
  // If threshold is 60 KB, msg_001 is 50 KB and msg_002 is 20 KB (total 70 KB)
  // Without thread preservation, they might split. With thread preservation, conv_101 stays together
  const volumes = splitMessagesIntoVolumes(sampleMessages, {
    maxVolumeSizeBytes: 100 * 1024, // 100 KB
    preserveThreads: true,
    filePrefix: 'EnterpriseArchive',
    targetFormat: 'PST_UNICODE',
  });

  assert.ok(volumes.length >= 2, 'Should split into at least 2 volumes due to size limit');
  assert.equal(volumes[0].fileName, 'EnterpriseArchive_part001.pst');
  assert.equal(volumes[1].fileName, 'EnterpriseArchive_part002.pst');

  // Check that msg_001 and msg_002 remain in the same volume because they share conversationId 'conv_101'
  const vol1HasMsg1 = volumes[0].messages.some(m => m.id === 'msg_001');
  const vol1HasMsg2 = volumes[0].messages.some(m => m.id === 'msg_002');
  assert.equal(vol1HasMsg1, vol1HasMsg2, 'Conversation thread messages should be together in the same volume');
});

test('DeltaSync: incremental checkpointing and deduplication', () => {
  // Initial run
  const diffInitial = calculateDeltaDiff(sampleMessages, null);
  assert.equal(diffInitial.newCount, 3);
  assert.equal(diffInitial.alreadyMigratedCount, 0);

  // Commit checkpoint for first 2 messages
  const checkpoint = createUpdatedCheckpoint(
    null,
    [sampleMessages[0], sampleMessages[1]],
    'source_mbox',
    'target_pst'
  );
  assert.equal(checkpoint.totalMessagesMigrated, 2);

  // Subsequent delta run with all 3 messages
  const diffDelta = calculateDeltaDiff(sampleMessages, checkpoint);
  assert.equal(diffDelta.alreadyMigratedCount, 2);
  assert.equal(diffDelta.newCount, 1);
  assert.equal(diffDelta.newMessages[0].id, 'msg_003');
});

test('Forensics: Bates stamping, deduplication, and chain of custody', () => {
  const stamped = applyBatesStamping(sampleMessages, 'ACME-CORP', 5000);
  assert.equal(stamped[0].batesNumber, 'ACME-CORP-0005000');
  assert.equal(stamped[1].batesNumber, 'ACME-CORP-0005001');

  // De-duplication check
  const withDuplicate = [...sampleMessages, { ...sampleMessages[0], id: 'msg_001_copy' }];
  const dedup = deduplicateMessages(withDuplicate, 'sha256');
  assert.equal(dedup.unique.length, 3);
  assert.equal(dedup.duplicates.length, 1);

  // Manifest verification
  const manifest = generateForensicManifest(stamped, 'CASE-2026-99', 'Detective Vance', 'mbox_dump', 'output.pst');
  assert.equal(manifest.totalItems, 3);
  assert.equal(manifest.batesRange.start, 'ACME-CORP-0005000');
  assert.ok(manifest.rootSha256.length > 0);
});

test('Direct PST Generator: MS-PST Unicode binary structure', () => {
  const result = buildDirectPst(sampleFolders, sampleMessages);
  assert.ok(result.binaryBuffer.length >= 512, 'Binary buffer must be at least 512 bytes (MS-PST Header)');
  
  // Verify magic bytes: 0x4E44424E ('!BDN')
  const view = new DataView(result.binaryBuffer.buffer);
  const magic = view.getUint32(0, false);
  assert.equal(magic, 0x4E44424E, 'PST Magic must be 0x4E44424E (!BDN for Unicode)');

  // Verify wVer is 23 (Unicode PST)
  const ver = view.getUint16(0x0A, true);
  assert.equal(ver, 23, 'PST version must be 23');

  // Verify wMagicClient is 0x534D ('SM')
  const magicClient = view.getUint16(0x08, true);
  assert.equal(magicClient, 0x534D, 'PST magic client must be 0x534D');
});
