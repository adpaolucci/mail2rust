import { EmailMessage, ForensicManifest } from '../types/mail';

/**
 * Fast synchronous SHA-256 / hash simulator for pure unit tests & web runtime
 */
export function simpleHash(text: string): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < text.length; i++) {
    const ch = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ ch, 2654435761);
    h2 = Math.imul(h2 ^ ch, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  const part1 = (h1 >>> 0).toString(16).padStart(8, '0');
  const part2 = (h2 >>> 0).toString(16).padStart(8, '0');
  const part3 = ((h1 ^ h2) >>> 0).toString(16).padStart(8, '0');
  const part4 = ((h1 + h2) >>> 0).toString(16).padStart(8, '0');
  return `${part1}${part2}${part3}${part4}`;
}

/**
 * Apply Bates stamping to an array of messages
 */
export function applyBatesStamping(
  messages: EmailMessage[],
  prefix: string = 'LEGAL-M2M',
  startingIndex: number = 1000
): EmailMessage[] {
  return messages.map((msg, index) => {
    const batesNumber = `${prefix}-${String(startingIndex + index).padStart(7, '0')}`;
    return {
      ...msg,
      batesNumber,
    };
  });
}

/**
 * Perform exact de-duplication based on SHA-256 or Message-ID
 */
export function deduplicateMessages(
  messages: EmailMessage[],
  strategy: 'sha256' | 'messageId' | 'strictBody' = 'sha256'
): { unique: EmailMessage[]; duplicates: EmailMessage[] } {
  const seen = new Set<string>();
  const unique: EmailMessage[] = [];
  const duplicates: EmailMessage[] = [];

  for (const msg of messages) {
    let key = '';
    if (strategy === 'sha256') {
      key = msg.sha256;
    } else if (strategy === 'messageId') {
      key = msg.messageId;
    } else {
      key = simpleHash(`${msg.from.email}|${msg.subject}|${msg.bodyText}`);
    }

    if (seen.has(key)) {
      duplicates.push(msg);
    } else {
      seen.add(key);
      unique.push(msg);
    }
  }

  return { unique, duplicates };
}

/**
 * Generates an immutable forensic chain of custody manifest
 */
export function generateForensicManifest(
  messages: EmailMessage[],
  caseNumber: string,
  examiner: string,
  sourceContainer: string,
  targetContainer: string
): ForensicManifest {
  const combinedHashes = messages.map(m => m.sha256).sort().join('');
  const rootSha256 = simpleHash(combinedHashes);
  const totalSizeBytes = messages.reduce((acc, m) => acc + m.sizeBytes, 0);

  const startBates = messages[0]?.batesNumber || 'N/A';
  const endBates = messages[messages.length - 1]?.batesNumber || 'N/A';

  return {
    caseNumber,
    examiner,
    timestamp: new Date().toISOString(),
    sourceContainer,
    targetContainer,
    totalItems: messages.length,
    totalSizeBytes,
    rootSha256,
    batesRange: { start: startBates, end: endBates },
    toolVersion: 'mail2mail-forensic-engine v3.2.0 (Direct MAPI / Rust)',
  };
}
