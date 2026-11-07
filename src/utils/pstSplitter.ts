import { EmailMessage, SplitConfig, SplitVolume } from '../types/mail';

/**
 * Splits a collection of messages across multiple PST volumes based on target size.
 * Optionally groups conversation threads together to avoid orphaning replies.
 */
export function splitMessagesIntoVolumes(
  messages: EmailMessage[],
  config: SplitConfig
): SplitVolume[] {
  if (messages.length === 0) {
    return [];
  }

  const { maxVolumeSizeBytes, preserveThreads, filePrefix } = config;
  const volumes: SplitVolume[] = [];

  let currentVolumeIndex = 1;
  let currentVolumeMessages: EmailMessage[] = [];
  let currentVolumeBytes = 0;
  let currentFolderSummary: Record<string, number> = {};

  // Helper to commit current volume
  const commitVolume = () => {
    if (currentVolumeMessages.length > 0) {
      const paddedIndex = String(currentVolumeIndex).padStart(3, '0');
      const fileName = `${filePrefix}_part${paddedIndex}.pst`;
      volumes.push({
        volumeIndex: currentVolumeIndex,
        fileName,
        messages: [...currentVolumeMessages],
        totalSizeBytes: currentVolumeBytes,
        folderSummary: { ...currentFolderSummary },
      });
      currentVolumeIndex++;
      currentVolumeMessages = [];
      currentVolumeBytes = 0;
      currentFolderSummary = {};
    }
  };

  if (preserveThreads) {
    // Group messages by conversation ID
    const threadMap = new Map<string, EmailMessage[]>();
    for (const msg of messages) {
      const threadId = msg.conversationId || msg.messageId || msg.id;
      if (!threadMap.has(threadId)) {
        threadMap.set(threadId, []);
      }
      threadMap.get(threadId)!.push(msg);
    }

    for (const [, threadMessages] of threadMap.entries()) {
      const threadSizeBytes = threadMessages.reduce((sum, m) => sum + m.sizeBytes, 0);

      // If adding this thread exceeds capacity and current volume is not empty, start a new volume
      if (currentVolumeBytes + threadSizeBytes > maxVolumeSizeBytes && currentVolumeMessages.length > 0) {
        commitVolume();
      }

      // Add all thread messages to current volume
      for (const msg of threadMessages) {
        currentVolumeMessages.push(msg);
        currentVolumeBytes += msg.sizeBytes;
        currentFolderSummary[msg.folderId] = (currentFolderSummary[msg.folderId] || 0) + 1;
      }
    }
  } else {
    // Strict item-by-item splitting
    for (const msg of messages) {
      if (currentVolumeBytes + msg.sizeBytes > maxVolumeSizeBytes && currentVolumeMessages.length > 0) {
        commitVolume();
      }

      currentVolumeMessages.push(msg);
      currentVolumeBytes += msg.sizeBytes;
      currentFolderSummary[msg.folderId] = (currentFolderSummary[msg.folderId] || 0) + 1;
    }
  }

  // Commit remaining volume
  commitVolume();

  return volumes;
}
