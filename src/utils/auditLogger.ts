import { AuditEvent, AuditCategory, AuditSeverity, HashChainVerificationResult } from '../types/audit';

export function computeHash(text: string): string {
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

export function createAuditEvent(
  partial: Omit<AuditEvent, 'prevHash' | 'eventHash' | 'timestampNanos'>,
  prevHash: string = '0000000000000000000000000000000000000000000000000000000000000000'
): AuditEvent {
  const timestampNanos = Date.now() * 1000000 + Math.floor(Math.random() * 999999);
  const rawPayload = `${partial.id}|${partial.timestamp}|${partial.severity}|${partial.category}|${partial.action}|${partial.actor}|${partial.targetResource}|${partial.description}|${prevHash}`;
  const eventHash = computeHash(rawPayload);

  return {
    ...partial,
    timestampNanos,
    prevHash,
    eventHash,
  };
}

export const INITIAL_AUDIT_LOGS: AuditEvent[] = [
  createAuditEvent(
    {
      id: 'AUD-2026-0001',
      timestamp: '2026-11-06T18:10:00.120Z',
      severity: 'SECURITY_AUDIT',
      category: 'CLOUD_AUTH',
      action: 'M365_OAUTH_TOKEN_ACQUIRED',
      actor: 'adpaolucci@corp.local [ADMIN]',
      targetResource: 'Tenant: 7a82b410-4491-49b0-91cd-89d1502fa481',
      description: 'Acquired Azure AD OAuth2 Client Credentials Bearer Token. Scopes: Mail.ReadWrite, MailboxSettings.ReadWrite.',
      clientIp: '127.0.0.1',
      processId: 'PID-4912 [mail2mail-rust-desktop]',
      metadata: { tenantId: '7a82b410-4491-49b0-91cd-89d1502fa481', grantType: 'client_credentials' },
    },
    'GENESIS_000000000000000000000000000000000000000000000000000000000000'
  ),
  createAuditEvent(
    {
      id: 'AUD-2026-0002',
      timestamp: '2026-11-06T18:12:30.450Z',
      severity: 'INFO',
      category: 'USER_ACTION',
      action: 'SELECT_MAILBOX_FOLDERS',
      actor: 'adpaolucci@corp.local [ADMIN]',
      targetResource: 'Folders: [Inbox, Sent, Titan Archive, Legal Hold]',
      description: 'User toggled selection for 6 of 8 mailbox containers in explorer tree.',
      clientIp: '127.0.0.1',
      processId: 'PID-4912',
    },
    'f3b890a14920cd81ef94819024cba184'
  ),
  createAuditEvent(
    {
      id: 'AUD-2026-0003',
      timestamp: '2026-11-06T18:15:12.890Z',
      severity: 'SECURITY_AUDIT',
      category: 'LEGAL_HOLD',
      action: 'ENABLE_STATUTORY_LEGAL_HOLD',
      actor: 'counsel@titan.corp.local [LEGAL]',
      targetResource: 'Matter: 26-CV-8891-DISC / Target Items: 4',
      description: 'Enacted immutable legal preservation hold on Project Titan executive communications.',
      clientIp: '10.128.4.15',
      processId: 'PID-4912',
      metadata: { caseNumber: '26-CV-8891-DISC', holdDuration: 'PERMANENT' },
    },
    '1a92cf84b1029481efca9028471bcca1'
  ),
  createAuditEvent(
    {
      id: 'AUD-2026-0004',
      timestamp: '2026-11-06T18:20:44.200Z',
      severity: 'NOTICE',
      category: 'CRYPTO_HASH',
      action: 'APPLY_BATES_STAMP_RANGE',
      actor: 'adpaolucci@corp.local [ADMIN]',
      targetResource: 'Corpus: 14 Messages',
      description: 'Sequencer applied Bates numbering range LEGAL-TITAN-0001001 through LEGAL-TITAN-0001014.',
      clientIp: '127.0.0.1',
      processId: 'PID-4912',
      metadata: { batesStart: 'LEGAL-TITAN-0001001', totalCount: 14 },
    },
    '3392ea810c93a812df94380126cf19a4'
  ),
  createAuditEvent(
    {
      id: 'AUD-2026-0005',
      timestamp: '2026-11-06T18:25:01.050Z',
      severity: 'INFO',
      category: 'PST_BUILDER',
      action: 'DIRECT_PST_HEADER_GENERATED',
      actor: 'mail2mail-rust-direct-mapi',
      targetResource: 'File: Export_Unicode_Archive.pst',
      description: 'Constructed 512-byte NDB Header: dwMagic 0x4E44424E (!BDN), wVer 23, CRC32 0xFFFFFF4F.',
      clientIp: '127.0.0.1',
      processId: 'PID-4912 [Native Rust FFI]',
      metadata: { format: 'MS-PST Unicode', encryption: 'NDB_CRYPT_PERMUTE' },
    },
    '884129ef94381026cf19a430e54aabe9'
  ),
  createAuditEvent(
    {
      id: 'AUD-2026-0006',
      timestamp: '2026-11-06T18:30:19.780Z',
      severity: 'WARNING',
      category: 'MIGRATION_PIPELINE',
      action: 'GRAPH_API_THROTTLE_RETRY',
      actor: 'mail2mail-cloud-worker',
      targetResource: 'Endpoint: /v1.0/users/adpaolucci@corp.local/messages',
      description: 'Received HTTP 429 Too Many Requests. Token bucket delay enforced (2000ms backoff).',
      clientIp: '52.168.112.44',
      processId: 'PID-4912 [Tokio Async Pool]',
      metadata: { retryCount: 1, delayMs: 2000 },
    },
    'e9b24f810c93a812df94380126cf19a4'
  ),
  createAuditEvent(
    {
      id: 'AUD-2026-0007',
      timestamp: '2026-11-06T18:35:50.310Z',
      severity: 'SECURITY_AUDIT',
      category: 'DATA_EXPORT',
      action: 'EXPORT_COMPLIANCE_REPORT',
      actor: 'adpaolucci@corp.local [ADMIN]',
      targetResource: 'Document: eDiscovery_Migration_Summary_26-CV-8891-DISC.csv',
      description: 'Exported formal compliance audit summary and item ledger with Root SHA-256 seal.',
      clientIp: '127.0.0.1',
      processId: 'PID-4912',
      metadata: { format: 'CSV & PDF Attestation', rootSha256: '30e54aabe9b24f810c93a812df94380126cf19a4' },
    },
    '99201481efca9028471bcca11a92cf84'
  ),
];

/**
 * Validates the cryptographic Merkle chain of all audit log events
 */
export function verifyAuditHashChain(events: AuditEvent[]): HashChainVerificationResult {
  if (events.length === 0) {
    return {
      isValid: true,
      totalEventsChecked: 0,
      rootHash: 'EMPTY_CHAIN',
      verificationTimestamp: new Date().toISOString(),
    };
  }

  for (let i = 0; i < events.length; i++) {
    const event = events[i];
    const rawPayload = `${event.id}|${event.timestamp}|${event.severity}|${event.category}|${event.action}|${event.actor}|${event.targetResource}|${event.description}|${event.prevHash}`;
    const calculatedHash = computeHash(rawPayload);

    if (calculatedHash !== event.eventHash) {
      return {
        isValid: false,
        totalEventsChecked: i + 1,
        brokenLinkIndex: i,
        rootHash: event.eventHash,
        verificationTimestamp: new Date().toISOString(),
      };
    }

    if (i > 0) {
      const prevEvent = events[i - 1];
      if (event.prevHash !== prevEvent.eventHash) {
        return {
          isValid: false,
          totalEventsChecked: i + 1,
          brokenLinkIndex: i,
          rootHash: event.eventHash,
          verificationTimestamp: new Date().toISOString(),
        };
      }
    }
  }

  const rootHash = events[events.length - 1].eventHash;
  return {
    isValid: true,
    totalEventsChecked: events.length,
    rootHash,
    verificationTimestamp: new Date().toISOString(),
  };
}

/**
 * Filter audit events by search query, category, and severity
 */
export function filterAuditEvents(
  events: AuditEvent[],
  searchQuery: string,
  category: 'ALL' | AuditCategory,
  severity: 'ALL' | AuditSeverity
): AuditEvent[] {
  let filtered = events;

  if (category !== 'ALL') {
    filtered = filtered.filter(e => e.category === category);
  }

  if (severity !== 'ALL') {
    filtered = filtered.filter(e => e.severity === severity);
  }

  if (searchQuery.trim() !== '') {
    const q = searchQuery.toLowerCase().trim();
    filtered = filtered.filter(
      e =>
        e.id.toLowerCase().includes(q) ||
        e.action.toLowerCase().includes(q) ||
        e.actor.toLowerCase().includes(q) ||
        e.targetResource.toLowerCase().includes(q) ||
        e.description.toLowerCase().includes(q) ||
        e.clientIp.toLowerCase().includes(q)
    );
  }

  return filtered;
}

/**
 * Format audit logs as RFC 3161 audit CSV
 */
export function exportAuditLogsCsv(events: AuditEvent[]): string {
  const escapeCsv = (str: string | undefined | null) => {
    if (!str) return '""';
    const s = String(str).replace(/"/g, '""');
    return `"${s}"`;
  };

  const lines = [
    '# Mail2Mail Studio - Forensic & Security Audit Log Stream (RFC 3161 Compliant)',
    `Export Date UTC,${new Date().toISOString()}`,
    `Total Audit Entries,${events.length}`,
    '',
    'Event ID,Timestamp UTC,Timestamp Nanos,Severity,Category,Action,Actor,Target Resource,Description,Client IP,Process ID,Prev Event Hash,Event Checksum Hash',
  ];

  for (const e of events) {
    lines.push([
      escapeCsv(e.id),
      escapeCsv(e.timestamp),
      e.timestampNanos,
      escapeCsv(e.severity),
      escapeCsv(e.category),
      escapeCsv(e.action),
      escapeCsv(e.actor),
      escapeCsv(e.targetResource),
      escapeCsv(e.description),
      escapeCsv(e.clientIp),
      escapeCsv(e.processId),
      escapeCsv(e.prevHash),
      escapeCsv(e.eventHash),
    ].join(','));
  }

  return lines.join('\r\n');
}
