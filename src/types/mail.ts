/**
 * Email Message and Folder Data Types for Mail2Mail Studio Desktop
 */

export interface EmailAttachment {
  id: string;
  filename: string;
  sizeBytes: number;
  mimeType: string;
  sha256: string;
  contentBase64?: string;
}

export interface MapiProperty {
  tag: string; // e.g. "0x0037001F"
  name: string; // e.g. "PR_SUBJECT_W"
  type: string; // e.g. "PT_UNICODE"
  value: string;
}

export interface EmailMessage {
  id: string;
  messageId: string; // RFC 822 Message-ID
  conversationId?: string;
  folderId: string;
  subject: string;
  from: {
    name: string;
    email: string;
  };
  to: Array<{ name: string; email: string }>;
  cc?: Array<{ name: string; email: string }>;
  bcc?: Array<{ name: string; email: string }>;
  date: string; // ISO string
  sizeBytes: number;
  bodyText: string;
  bodyHtml?: string;
  rawMime?: string;
  attachments: EmailAttachment[];
  mapiProperties: MapiProperty[];
  isRead: boolean;
  isFlagged: boolean;
  importance: 'low' | 'normal' | 'high';
  // Forensic properties
  sha256: string;
  md5: string;
  batesNumber?: string;
  legalHold?: boolean;
  sourceFormat: 'PST' | 'OST' | 'MBOX' | 'EML' | 'MSG' | 'DBX' | 'NSF' | 'EMLX' | 'MAILDIR' | 'CLOUD_M365' | 'CLOUD_GWORKSPACE' | 'CLOUD_IMAP';
}

export interface MailboxFolder {
  id: string;
  parentId: string | null;
  name: string;
  folderType: 'inbox' | 'sent' | 'drafts' | 'trash' | 'archive' | 'custom' | 'calendar' | 'contacts';
  unreadCount: number;
  totalCount: number;
  sizeBytes: number;
  selected?: boolean;
}

export interface FilterCriteria {
  startDate?: string | null; // ISO Date YYYY-MM-DD
  endDate?: string | null;   // ISO Date YYYY-MM-DD
  senderSearch?: string;
  recipientSearch?: string;
  subjectSearch?: string;
  hasAttachments?: boolean | null;
  minSizeBytes?: number | null;
  maxSizeBytes?: number | null;
  importance?: 'all' | 'low' | 'normal' | 'high';
  readStatus?: 'all' | 'read' | 'unread';
  flaggedOnly?: boolean;
  legalHoldOnly?: boolean;
}

export interface SplitConfig {
  maxVolumeSizeBytes: number; // e.g. 20GB = 20 * 1024 * 1024 * 1024
  preserveThreads: boolean;
  filePrefix: string;
  targetFormat: 'PST_UNICODE' | 'PST_ANSI';
}

export interface SplitVolume {
  volumeIndex: number;
  fileName: string;
  messages: EmailMessage[];
  totalSizeBytes: number;
  folderSummary: Record<string, number>; // folderId -> count
}

export interface SyncCheckpoint {
  id: string;
  timestamp: string;
  sourceId: string;
  targetId: string;
  migratedMessageIds: string[]; // Hash or Message-ID
  folderHighUids: Record<string, number>;
  totalBytesMigrated: number;
  totalMessagesMigrated: number;
  status: 'completed' | 'in_progress' | 'failed';
}

export interface ForensicManifest {
  caseNumber: string;
  examiner: string;
  timestamp: string;
  sourceContainer: string;
  targetContainer: string;
  totalItems: number;
  totalSizeBytes: number;
  rootSha256: string;
  batesRange: { start: string; end: string };
  toolVersion: string;
}
