import { EmailMessage, MailboxFolder } from '../types/mail';

export interface BulkDeleteResult {
  remainingMessages: EmailMessage[];
  deletedMessages: EmailMessage[];
  blockedLegalHoldCount: number;
}

/**
 * Pure bulk deletion logic with legal hold compliance safeguards
 */
export function bulkDeleteMessages(
  messages: EmailMessage[],
  idsToDelete: string[],
  options: { allowLegalHoldDeletion?: boolean } = {}
): BulkDeleteResult {
  const toDeleteSet = new Set(idsToDelete);
  const remainingMessages: EmailMessage[] = [];
  const deletedMessages: EmailMessage[] = [];
  let blockedLegalHoldCount = 0;

  for (const msg of messages) {
    if (toDeleteSet.has(msg.id)) {
      if (msg.legalHold && !options.allowLegalHoldDeletion) {
        // Protected by legal hold
        blockedLegalHoldCount++;
        remainingMessages.push(msg);
      } else {
        deletedMessages.push(msg);
      }
    } else {
      remainingMessages.push(msg);
    }
  }

  return {
    remainingMessages,
    deletedMessages,
    blockedLegalHoldCount,
  };
}

/**
 * Recomputes folder statistics (totalCount, unreadCount, sizeBytes) after message deletion
 */
export function updateFolderStatsAfterDeletion(
  folders: MailboxFolder[],
  deletedMessages: EmailMessage[]
): MailboxFolder[] {
  if (deletedMessages.length === 0) return folders;

  // Group deleted messages by folderId
  const deletedByFolder = new Map<string, { count: number; unread: number; bytes: number }>();

  for (const msg of deletedMessages) {
    const current = deletedByFolder.get(msg.folderId) || { count: 0, unread: 0, bytes: 0 };
    current.count += 1;
    if (!msg.isRead) current.unread += 1;
    current.bytes += msg.sizeBytes;
    deletedByFolder.set(msg.folderId, current);
  }

  return folders.map((folder) => {
    const stats = deletedByFolder.get(folder.id);
    if (!stats) return folder;

    return {
      ...folder,
      totalCount: Math.max(0, folder.totalCount - stats.count),
      unreadCount: Math.max(0, folder.unreadCount - stats.unread),
      sizeBytes: Math.max(0, folder.sizeBytes - stats.bytes),
    };
  });
}
