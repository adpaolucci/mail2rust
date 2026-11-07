import test from 'node:test';
import assert from 'node:assert/strict';
import { bulkDeleteMessages, updateFolderStatsAfterDeletion } from './messageOps';
import { EmailMessage, MailboxFolder } from '../types/mail';

const mockMessages: EmailMessage[] = [
  {
    id: 'msg_01',
    messageId: '<01@domain.com>',
    conversationId: 'c1',
    folderId: 'inbox',
    subject: 'Standard message',
    from: { name: 'User A', email: 'a@domain.com' },
    to: [{ name: 'User B', email: 'b@domain.com' }],
    date: '2026-11-01T00:00:00Z',
    sizeBytes: 1000,
    bodyText: 'Hello',
    attachments: [],
    mapiProperties: [],
    isRead: false,
    isFlagged: false,
    importance: 'normal',
    sha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    md5: 'd41d8cd98f00b204e9800998ecf8427e',
    sourceFormat: 'PST',
  },
  {
    id: 'msg_02',
    messageId: '<02@domain.com>',
    conversationId: 'c2',
    folderId: 'inbox',
    subject: 'Protected Legal Hold Message',
    from: { name: 'Legal Counsel', email: 'legal@domain.com' },
    to: [{ name: 'User B', email: 'b@domain.com' }],
    date: '2026-11-02T00:00:00Z',
    sizeBytes: 2500,
    bodyText: 'Under discovery hold',
    attachments: [],
    mapiProperties: [],
    isRead: true,
    isFlagged: true,
    importance: 'high',
    legalHold: true,
    sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
    md5: '098f6bcd4621d373cade4e832627b4f6',
    sourceFormat: 'PST',
  },
  {
    id: 'msg_03',
    messageId: '<03@domain.com>',
    conversationId: 'c3',
    folderId: 'sent',
    subject: 'Sent mail',
    from: { name: 'User B', email: 'b@domain.com' },
    to: [{ name: 'User C', email: 'c@domain.com' }],
    date: '2026-11-03T00:00:00Z',
    sizeBytes: 4000,
    bodyText: 'Sent',
    attachments: [],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'normal',
    sha256: '5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8',
    md5: '5d41402abc4b2a76b9719d911017c592',
    sourceFormat: 'PST',
  },
];

const mockFolders: MailboxFolder[] = [
  { id: 'inbox', parentId: null, name: 'Inbox', folderType: 'inbox', unreadCount: 1, totalCount: 2, sizeBytes: 3500 },
  { id: 'sent', parentId: null, name: 'Sent Items', folderType: 'sent', unreadCount: 0, totalCount: 1, sizeBytes: 4000 },
];

test('MessageOps: bulk delete respects legal hold protection by default', () => {
  const result = bulkDeleteMessages(mockMessages, ['msg_01', 'msg_02']);

  // msg_02 is protected by legal hold
  assert.equal(result.remainingMessages.length, 2);
  assert.equal(result.deletedMessages.length, 1);
  assert.equal(result.deletedMessages[0].id, 'msg_01');
  assert.equal(result.blockedLegalHoldCount, 1);
  assert.ok(result.remainingMessages.some(m => m.id === 'msg_02'));
});

test('MessageOps: bulk delete allows legal hold override when explicitly specified', () => {
  const result = bulkDeleteMessages(mockMessages, ['msg_01', 'msg_02'], { allowLegalHoldDeletion: true });

  assert.equal(result.remainingMessages.length, 1);
  assert.equal(result.deletedMessages.length, 2);
  assert.equal(result.blockedLegalHoldCount, 0);
  assert.equal(result.remainingMessages[0].id, 'msg_03');
});

test('MessageOps: updates folder counts and sizes after deletion', () => {
  const { deletedMessages } = bulkDeleteMessages(mockMessages, ['msg_01']);
  const updatedFolders = updateFolderStatsAfterDeletion(mockFolders, deletedMessages);

  const inbox = updatedFolders.find(f => f.id === 'inbox')!;
  assert.equal(inbox.totalCount, 1);
  assert.equal(inbox.unreadCount, 0);
  assert.equal(inbox.sizeBytes, 2500);

  // Sent folder was untouched
  const sent = updatedFolders.find(f => f.id === 'sent')!;
  assert.equal(sent.totalCount, 1);
  assert.equal(sent.sizeBytes, 4000);
});
