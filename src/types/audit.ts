export type AuditSeverity = 'INFO' | 'NOTICE' | 'WARNING' | 'SECURITY_AUDIT' | 'ERROR';

export type AuditCategory =
  | 'CRYPTO_HASH'
  | 'MIGRATION_PIPELINE'
  | 'LEGAL_HOLD'
  | 'PST_BUILDER'
  | 'CLOUD_AUTH'
  | 'USER_ACTION'
  | 'DATA_EXPORT';

export interface AuditEvent {
  id: string; // e.g. "AUD-2026-88412"
  timestamp: string; // ISO UTC
  timestampNanos: number;
  severity: AuditSeverity;
  category: AuditCategory;
  action: string; // e.g. "APPLY_BATES_STAMPING", "DIRECT_PST_BUILD_START"
  actor: string; // e.g. "adpaolucci@corp.local [ADMIN]"
  targetResource: string; // e.g. "Folder: Archive (14 items)"
  description: string;
  clientIp: string;
  processId: string;
  metadata?: Record<string, any>;
  prevHash: string; // Cryptographic chain of previous event
  eventHash: string; // SHA-256 of (id + timestamp + action + actor + prevHash)
}

export interface HashChainVerificationResult {
  isValid: boolean;
  totalEventsChecked: number;
  brokenLinkIndex?: number;
  rootHash: string;
  verificationTimestamp: string;
}
