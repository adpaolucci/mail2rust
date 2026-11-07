import test from 'node:test';
import assert from 'node:assert/strict';
import {
  computeMd5,
  computeSha1,
  computeSha256,
  calculateForensicHashes,
} from './cryptoHashes';
import { EmailMessage } from '../types/mail';

test('CryptoHashes: verifies standard RFC test vectors for empty string', () => {
  assert.equal(computeMd5(''), 'd41d8cd98f00b204e9800998ecf8427e');
  assert.equal(computeSha1(''), 'da39a3ee5e6b4b0d3255bfef95601890afd80709');
  assert.equal(computeSha256(''), 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
});

test('CryptoHashes: verifies standard test vectors for "The quick brown fox jumps over the lazy dog"', () => {
  const text = 'The quick brown fox jumps over the lazy dog';
  assert.equal(computeMd5(text), '9e107d9d372bb6826bd81d3542a419d6');
  assert.equal(computeSha1(text), '2fd4e1c67a2d28fced849ee1bb76e7391b93eb12');
  assert.equal(computeSha256(text), 'd7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592');
});

test('CryptoHashes: calculates MD5, SHA-1, and SHA-256 for email body and attachments', () => {
  const testMessage: EmailMessage = {
    id: 'test_msg_01',
    messageId: '<test.01@forensics.local>',
    conversationId: 'c_test_01',
    folderId: 'inbox',
    subject: 'Forensic Audit Report',
    from: { name: 'Investigator', email: 'sec@corp.local' },
    to: [{ name: 'Legal Counsel', email: 'legal@corp.local' }],
    date: '2026-11-07T00:00:00Z',
    sizeBytes: 8192,
    bodyText: 'Attached are the extracted mailbox forensic records.',
    bodyHtml: '<p>Attached are the extracted mailbox forensic records.</p>',
    rawMime: 'Subject: Forensic Audit Report\r\n\r\nAttached are the extracted mailbox forensic records.',
    attachments: [
      {
        id: 'att_01',
        filename: 'evidence_ledger.csv',
        sizeBytes: 4096,
        mimeType: 'text/csv',
        sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
      },
    ],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'high',
    sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
    md5: '098f6bcd4621d373cade4e832627b4f6',
    sourceFormat: 'PST',
  };

  const summary = calculateForensicHashes(testMessage);

  // Body Text Hashes
  assert.equal(summary.bodyText.md5.length, 32);
  assert.equal(summary.bodyText.sha1.length, 40);
  assert.equal(summary.bodyText.sha256.length, 64);
  assert.equal(summary.bodyText.type, 'body_text');

  // Body HTML Hashes
  assert.ok(summary.bodyHtml);
  assert.equal(summary.bodyHtml.md5.length, 32);
  assert.equal(summary.bodyHtml.sha1.length, 40);
  assert.equal(summary.bodyHtml.sha256.length, 64);

  // Attachments Hashes
  assert.equal(summary.attachments.length, 1);
  assert.equal(summary.attachments[0].name, 'evidence_ledger.csv');
  assert.equal(summary.attachments[0].md5.length, 32);
  assert.equal(summary.attachments[0].sha1.length, 40);
  assert.equal(summary.attachments[0].sha256.length, 64);

  // Composite Hash
  assert.equal(summary.compositeHash.md5.length, 32);
  assert.equal(summary.compositeHash.sha1.length, 40);
  assert.equal(summary.compositeHash.sha256.length, 64);
});
