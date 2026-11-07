import { EmailMessage, SyncCheckpoint } from '../types/mail';

export interface DeltaDiffResult {
  newMessages: EmailMessage[];
  alreadyMigratedCount: number;
  newCount: number;
  newSizeBytes: number;
}

/**
 * Calculates what items need migration given an existing checkpoint
 */
export function calculateDeltaDiff(
  sourceMessages: EmailMessage[],
  checkpoint: SyncCheckpoint | null
): DeltaDiffResult {
  if (!checkpoint || !checkpoint.migratedMessageIds || checkpoint.migratedMessageIds.length === 0) {
    const totalBytes = sourceMessages.reduce((sum, m) => sum + m.sizeBytes, 0);
    return {
      newMessages: sourceMessages,
      alreadyMigratedCount: 0,
      newCount: sourceMessages.length,
      newSizeBytes: totalBytes,
    };
  }

  const migratedSet = new Set(checkpoint.migratedMessageIds);
  const newMessages: EmailMessage[] = [];
  let newSizeBytes = 0;
  let alreadyMigratedCount = 0;

  for (const msg of sourceMessages) {
    // Unique identifier key: prefer messageId, fallback to sha256 or id
    const key = msg.messageId || msg.sha256 || msg.id;
    if (migratedSet.has(key)) {
      alreadyMigratedCount++;
    } else {
      newMessages.push(msg);
      newSizeBytes += msg.sizeBytes;
    }
  }

  return {
    newMessages,
    alreadyMigratedCount,
    newCount: newMessages.length,
    newSizeBytes,
  };
}

/**
 * Commits a migration run and generates an updated checkpoint
 */
export function createUpdatedCheckpoint(
  existingCheckpoint: SyncCheckpoint | null,
  migratedInThisRun: EmailMessage[],
  sourceId: string,
  targetId: string
): SyncCheckpoint {
  const previousIds = existingCheckpoint ? existingCheckpoint.migratedMessageIds : [];
  const newlyMigratedIds = migratedInThisRun.map(m => m.messageId || m.sha256 || m.id);

  const mergedIds = Array.from(new Set([...previousIds, ...newlyMigratedIds]));
  const additionalBytes = migratedInThisRun.reduce((sum, m) => sum + m.sizeBytes, 0);
  const previousBytes = existingCheckpoint ? existingCheckpoint.totalBytesMigrated : 0;

  // Track highest fake UID per folder
  const folderHighUids = { ...(existingCheckpoint?.folderHighUids || {}) };
  for (const msg of migratedInThisRun) {
    folderHighUids[msg.folderId] = (folderHighUids[msg.folderId] || 0) + 1;
  }

  return {
    id: `chk_${Date.now()}`,
    timestamp: new Date().toISOString(),
    sourceId,
    targetId,
    migratedMessageIds: mergedIds,
    folderHighUids,
    totalBytesMigrated: previousBytes + additionalBytes,
    totalMessagesMigrated: mergedIds.length,
    status: 'completed',
  };
}
