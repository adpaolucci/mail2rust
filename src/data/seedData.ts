import { EmailMessage, MailboxFolder } from '../types/mail';

export const INITIAL_FOLDERS: MailboxFolder[] = [
  { id: 'inbox', parentId: null, name: 'Inbox', folderType: 'inbox', unreadCount: 2, totalCount: 6, sizeBytes: 2840000, selected: true },
  { id: 'sent', parentId: null, name: 'Sent Items', folderType: 'sent', unreadCount: 0, totalCount: 3, sizeBytes: 1420000, selected: true },
  { id: 'drafts', parentId: null, name: 'Drafts', folderType: 'drafts', unreadCount: 1, totalCount: 1, sizeBytes: 320000, selected: false },
  { id: 'archive', parentId: null, name: 'Project Titan Archive', folderType: 'archive', unreadCount: 0, totalCount: 4, sizeBytes: 9800000, selected: true },
  { id: 'legal', parentId: 'archive', name: 'Legal & Compliance Hold', folderType: 'custom', unreadCount: 0, totalCount: 3, sizeBytes: 5200000, selected: true },
  { id: 'trash', parentId: null, name: 'Deleted Items', folderType: 'trash', unreadCount: 0, totalCount: 2, sizeBytes: 890000, selected: false },
  { id: 'calendar', parentId: null, name: 'Calendar / Appointments', folderType: 'calendar', unreadCount: 0, totalCount: 2, sizeBytes: 450000, selected: true },
  { id: 'contacts', parentId: null, name: 'Global Address Book', folderType: 'contacts', unreadCount: 0, totalCount: 2, sizeBytes: 180000, selected: true },
];

export const INITIAL_MESSAGES: EmailMessage[] = [
  {
    id: 'msg_001',
    messageId: '<20261101.98412@titan.corp.local>',
    conversationId: 'conv_titan_01',
    folderId: 'inbox',
    subject: 'Project Titan: Master Architecture Transition & Cloud Migration Spec',
    from: { name: 'Dr. Evelyn Vance', email: 'evelyn.vance@titan.corp.local' },
    to: [
      { name: 'Architecture Review Board', email: 'arb@titan.corp.local' },
      { name: 'Migration Lead', email: 'adpaolucci@corp.local' },
    ],
    cc: [{ name: 'Security Operations', email: 'secops@titan.corp.local' }],
    date: '2026-11-01T08:30:00Z',
    sizeBytes: 1024 * 480, // 480 KB
    bodyText: `Team,\n\nAttached is the completed architectural blueprint for our email migration pipeline.\nKey objectives include:\n1. Direct PST generation in Rust using MS-PST specifications (zero Outlook MAPI dependency).\n2. Dual-mode support for Microsoft 365 Graph and Google Workspace Domain-Wide Delegation.\n3. Automatic volume splitting at 20GB boundary with thread preservation.\n4. RFC3161 forensic chain of custody and Bates numbering.\n\nPlease review and approve by Friday.\n\nBest regards,\nEvelyn Vance, PhD\nChief Enterprise Architect`,
    bodyHtml: `<div style="font-family: 'Segoe UI', sans-serif; color: #f1f5f9; line-height: 1.6;">
      <h3 style="color: #60a5fa; margin-bottom: 12px; border-bottom: 2px solid #3b82f6; padding-bottom: 6px;">Project Titan: Enterprise Migration Specification</h3>
      <p>Team,</p>
      <p>Attached is the completed architectural blueprint for our email migration pipeline. Key objectives include:</p>
      <ul style="padding-left: 20px; color: #cbd5e1;">
        <li><strong style="color: #ffffff;">Direct PST generation in Rust</strong> using MS-PST specifications (zero Outlook MAPI dependency).</li>
        <li><strong style="color: #ffffff;">Dual-mode cloud support</strong> for Microsoft 365 Graph and Google Workspace Domain-Wide Delegation.</li>
        <li><strong style="color: #ffffff;">Automatic volume splitting</strong> at 20GB boundary with thread preservation.</li>
        <li><strong style="color: #ffffff;">RFC3161 forensic chain of custody</strong> and Bates numbering.</li>
      </ul>
      <p style="margin-top: 16px;">Please review and approve by Friday.</p>
      <hr style="border: none; border-top: 1px solid #334155; margin: 20px 0;" />
      <p style="color: #94a3b8; font-size: 13px;">
        <strong style="color: #e2e8f0;">Dr. Evelyn Vance, PhD</strong><br />
        Chief Enterprise Architect | Titan Technologies
      </p>
    </div>`,
    rawMime: `Delivered-To: adpaolucci@corp.local
Received: by 10.128.4.15 with SMTP id a15csp3429181
        for <adpaolucci@corp.local>; Fri, 1 Nov 2026 08:30:02 -0700 (PDT)
X-Google-Smtp-Source: AGHT+IF34Jjke9432KLDfk=
MIME-Version: 1.0
Message-ID: <20261101.98412@titan.corp.local>
Subject: Project Titan: Master Architecture Transition & Cloud Migration Spec
From: "Dr. Evelyn Vance" <evelyn.vance@titan.corp.local>
To: "Architecture Review Board" <arb@titan.corp.local>, "Migration Lead" <adpaolucci@corp.local>
Date: Fri, 01 Nov 2026 08:30:00 +0000
Content-Type: multipart/mixed; boundary="===============7894231904=="

--===============7894231904==
Content-Type: text/html; charset="utf-8"
Content-Transfer-Encoding: 7bit

[HTML Payload Omitted]
--===============7894231904==--`,
    attachments: [
      {
        id: 'att_001',
        filename: 'Titan_Architecture_v3.pdf',
        sizeBytes: 1024 * 390,
        mimeType: 'application/pdf',
        sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
      },
      {
        id: 'att_002',
        filename: 'MAPI_Property_Tags_MS_PST.xlsx',
        sizeBytes: 1024 * 65,
        mimeType: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
        sha256: 'a1b2c3d4e5f67890123456789abcdef0123456789abcdef0123456789abcdef0',
      },
    ],
    mapiProperties: [
      { tag: '0x001A001F', name: 'PR_MESSAGE_CLASS_W', type: 'PT_UNICODE', value: 'IPM.Note' },
      { tag: '0x0037001F', name: 'PR_SUBJECT_W', type: 'PT_UNICODE', value: 'Project Titan: Master Architecture Transition' },
      { tag: '0x0C1F001F', name: 'PR_SENDER_NAME_W', type: 'PT_UNICODE', value: 'Dr. Evelyn Vance' },
      { tag: '0x0C1F001F', name: 'PR_SENDER_EMAIL_ADDRESS_W', type: 'PT_UNICODE', value: 'evelyn.vance@titan.corp.local' },
      { tag: '0x00390040', name: 'PR_CLIENT_SUBMIT_TIME', type: 'PT_SYSTIME', value: '2026-11-01T08:30:00.000Z' },
      { tag: '0x0E070003', name: 'PR_MESSAGE_FLAGS', type: 'PT_LONG', value: '0x00000001 (MSGFLAG_READ)' },
      { tag: '0x00170003', name: 'PR_IMPORTANCE', type: 'PT_LONG', value: '0x00000002 (HIGH)' },
      { tag: '0x0FF90102', name: 'PR_RECORD_KEY', type: 'PT_BINARY', value: 'D4 8F 21 AA 93 42 10 99 BB C1 00 24' },
      { tag: '0x0E080003', name: 'PR_MESSAGE_SIZE', type: 'PT_LONG', value: '491520 bytes' },
    ],
    isRead: true,
    isFlagged: true,
    importance: 'high',
    sha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    md5: 'c4ca4238a0b923820dcc509a6f75849b',
    batesNumber: 'SEC-TITAN-0001001',
    legalHold: true,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_002',
    messageId: '<20261102.11204@titan.corp.local>',
    conversationId: 'conv_titan_01',
    folderId: 'inbox',
    subject: 'Re: Project Titan: Master Architecture Transition & Cloud Migration Spec',
    from: { name: 'Marcus Sterling', email: 'm.sterling@titan.corp.local' },
    to: [{ name: 'Dr. Evelyn Vance', email: 'evelyn.vance@titan.corp.local' }],
    date: '2026-11-02T10:15:00Z',
    sizeBytes: 1024 * 120, // 120 KB
    bodyText: `Evelyn,\n\nReviewed the PST parser specification. The decision to implement an in-process Rust NDB/LTP engine instead of relying on the legacy Outlook MAPI client is a major win for Linux and containerized headless migrations.\n\nWe tested the 45 GB volume boundary with 10,000 synthetic items. Split logic correctly maintained thread coherence without splitting conv_titan_01.\n\nReady for staging.\n\nMarcus`,
    bodyHtml: `<div style="font-family: 'Segoe UI', sans-serif; color: #f1f5f9; line-height: 1.6;">
      <p>Evelyn,</p>
      <p>Reviewed the PST parser specification. The decision to implement an in-process <strong style="color: #60a5fa;">Rust NDB/LTP engine</strong> instead of relying on the legacy Outlook MAPI client is a major win for Linux and containerized headless migrations.</p>
      <p>We tested the <strong style="color: #60a5fa;">45 GB volume boundary</strong> with 10,000 synthetic items. Split logic correctly maintained thread coherence without splitting <code style="background: #1e293b; padding: 2px 6px; border-radius: 4px; color: #38bdf8;">conv_titan_01</code>.</p>
      <p style="margin-top: 12px;">Ready for staging.</p>
      <p style="color: #94a3b8; margin-top: 16px;">— Marcus</p>
    </div>`,
    rawMime: `Delivered-To: evelyn.vance@titan.corp.local
Message-ID: <20261102.11204@titan.corp.local>
In-Reply-To: <20261101.98412@titan.corp.local>
Subject: Re: Project Titan: Master Architecture Transition & Cloud Migration Spec
From: "Marcus Sterling" <m.sterling@titan.corp.local>
To: "Dr. Evelyn Vance" <evelyn.vance@titan.corp.local>
Date: Sat, 02 Nov 2026 10:15:00 +0000
Content-Type: text/html; charset="utf-8"`,
    attachments: [],
    mapiProperties: [
      { tag: '0x001A001F', name: 'PR_MESSAGE_CLASS_W', type: 'PT_UNICODE', value: 'IPM.Note' },
      { tag: '0x0037001F', name: 'PR_SUBJECT_W', type: 'PT_UNICODE', value: 'Re: Project Titan: Master Architecture Transition' },
      { tag: '0x00170003', name: 'PR_IMPORTANCE', type: 'PT_LONG', value: '0x00000001 (NORMAL)' },
    ],
    isRead: false,
    isFlagged: false,
    importance: 'normal',
    sha256: '5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8',
    md5: 'c81e728d9d4c2f636f067f89cc14862c',
    batesNumber: 'SEC-TITAN-0001002',
    legalHold: true,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_003',
    messageId: '<20261103.88219@cloud-gateway.microsoft.com>',
    conversationId: 'conv_cloud_02',
    folderId: 'inbox',
    subject: 'Microsoft Graph & Exchange Online Throttling Tier Upgraded (50 msg/sec)',
    from: { name: 'Microsoft 365 Cloud Ops', email: 'no-reply@microsoft.com' },
    to: [{ name: 'Enterprise Cloud Admin', email: 'cloudadmin@titan.corp.local' }],
    date: '2026-11-03T14:45:00Z',
    sizeBytes: 1024 * 85,
    bodyText: `Your tenant subscription (ID: 7a82b410-4491-49b0-91cd-89d1502fa481) has been provisioned with elevated migration concurrency.\nBatch import limit: 50 requests/sec with exponential backoff on HTTP 429 Retry-After.\nGraph Scope: Mail.ReadWrite, MailboxSettings.ReadWrite granted.`,
    attachments: [],
    mapiProperties: [
      { tag: '0x001A001F', name: 'PR_MESSAGE_CLASS_W', type: 'PT_UNICODE', value: 'IPM.Note' },
      { tag: '0x0037001F', name: 'PR_SUBJECT_W', type: 'PT_UNICODE', value: 'Microsoft Graph & Exchange Online Throttling Tier Upgraded' },
    ],
    isRead: true,
    isFlagged: false,
    importance: 'high',
    sha256: '4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a',
    md5: 'eccbc87e4b5ce2fe28308fd9f2a7baf3',
    batesNumber: 'SEC-TITAN-0001003',
    legalHold: false,
    sourceFormat: 'CLOUD_M365',
  },
  {
    id: 'msg_004',
    messageId: '<20261104.55102@google.com>',
    conversationId: 'conv_gsuite_03',
    folderId: 'inbox',
    subject: 'Google Workspace: Service Account JWT with Domain-Wide Delegation Active',
    from: { name: 'Google Workspace Admin', email: 'workspace-noreply@google.com' },
    to: [{ name: 'System Integrator', email: 'integrator@titan.corp.local' }],
    date: '2026-11-04T16:00:00Z',
    sizeBytes: 1024 * 160,
    bodyText: `Domain-Wide Delegation has been verified for service account: mail2mail-sa@titan-cloud-prod.iam.gserviceaccount.com.\nAuthorized OAuth Scopes: https://mail.google.com/, https://www.googleapis.com/auth/gmail.modify.\nDirect REST sync rate limit: 250 quota units/sec.`,
    attachments: [
      {
        id: 'att_003',
        filename: 'service-account-auth-manifest.json',
        sizeBytes: 1024 * 4,
        mimeType: 'application/json',
        sha256: 'ef2d127de37b942baad06145e54b0c619a1f22327b2ebbcfbec78f5564afe39d',
      },
    ],
    mapiProperties: [],
    isRead: false,
    isFlagged: true,
    importance: 'normal',
    sha256: 'ef2d127de37b942baad06145e54b0c619a1f22327b2ebbcfbec78f5564afe39d',
    md5: 'a87ff679a2f3e71d9181a67b7542122c',
    batesNumber: 'SEC-TITAN-0001004',
    legalHold: false,
    sourceFormat: 'CLOUD_GWORKSPACE',
  },
  {
    id: 'msg_005',
    messageId: '<20261028.77441@notes-server.legacy.net>',
    conversationId: 'conv_nsf_01',
    folderId: 'inbox',
    subject: 'Lotus Notes / IBM Domino Mailbox Archive Dump: Q4 Financials (.nsf)',
    from: { name: 'Legacy Archive Daemon', email: 'nsf-export@legacy.net' },
    to: [{ name: 'Records Retention', email: 'retention@titan.corp.local' }],
    date: '2026-10-28T11:20:00Z',
    sizeBytes: 1024 * 1100, // 1.1 MB
    bodyText: `Extracted from Lotus Notes 8.5 Domino NSF database. Converted via mail2mail ODS (On-Disk Structure) parser.\nContains 28 financial worksheets and audit memos from fiscal year 2018-2022.`,
    attachments: [
      {
        id: 'att_004',
        filename: 'domino_ledger_extract.csv',
        sizeBytes: 1024 * 900,
        mimeType: 'text/csv',
        sha256: '8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4',
      },
    ],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'low',
    sha256: '8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4',
    md5: 'e4da3b7fbbce2345d7772b0674a318d5',
    batesNumber: 'SEC-TITAN-0001005',
    legalHold: true,
    sourceFormat: 'NSF',
  },
  {
    id: 'msg_006',
    messageId: '<20261020.19830@outlookexpress.local>',
    conversationId: 'conv_dbx_01',
    folderId: 'inbox',
    subject: 'Historical Outlook Express 6.0 DBX Archive: Exec Correspondence',
    from: { name: 'Archival Vault', email: 'vault@titan.corp.local' },
    to: [{ name: 'Legal Hold Repository', email: 'legal@titan.corp.local' }],
    date: '2026-10-20T15:10:00Z',
    sizeBytes: 1024 * 890,
    bodyText: `Source container: OE6_Inbox.dbx\nSignature: CF AD 12 FE (DBX File Header)\nDirectly converted to standard MIME and PST NDB without running Windows XP or Outlook Express.`,
    attachments: [],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'normal',
    sha256: '3448a97992cbfa0930054a4ef01aa240483ebe9aa6ec3439908e52a6013bc730',
    md5: '1679091c5a880faf6fb5e6087eb1b2dc',
    batesNumber: 'SEC-TITAN-0001006',
    legalHold: true,
    sourceFormat: 'DBX',
  },
  {
    id: 'msg_007',
    messageId: '<20261105.10111@titan.corp.local>',
    conversationId: 'conv_sent_01',
    folderId: 'sent',
    subject: 'Signed Retainer Agreement - Outside Legal Counsel',
    from: { name: 'General Counsel', email: 'counsel@titan.corp.local' },
    to: [{ name: 'Partner Attorney', email: 'partner@lawfirm.com' }],
    date: '2026-11-05T12:00:00Z',
    sizeBytes: 1024 * 720,
    bodyText: `Counsel,\n\nPlease find the counter-signed legal representation agreement attached.\nCryptographic SHA-256 seal has been applied.\n\nRegards,\nCorporate Legal Office`,
    attachments: [
      {
        id: 'att_005',
        filename: 'CounterSigned_Retainer_2026.pdf',
        sizeBytes: 1024 * 650,
        mimeType: 'application/pdf',
        sha256: '7b52009b64fd0a2a49e6d8a939753077792b0554cac5247074101e0a509447c4',
      },
    ],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'high',
    sha256: '7b52009b64fd0a2a49e6d8a939753077792b0554cac5247074101e0a509447c4',
    md5: '8f14e45fceea167a5a36dedd4bea2543',
    batesNumber: 'SEC-TITAN-0001007',
    legalHold: true,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_008',
    messageId: '<20261104.99120@titan.corp.local>',
    conversationId: 'conv_sent_02',
    folderId: 'sent',
    subject: 'eDiscovery Subpoena Response Packet: Case 26-CV-8891',
    from: { name: 'eDiscovery Director', email: 'ediscovery@titan.corp.local' },
    to: [{ name: 'District Court Clerk', email: 'filing@court.gov' }],
    date: '2026-11-04T18:20:00Z',
    sizeBytes: 1024 * 450,
    bodyText: `Pursuant to the discovery order in Case 26-CV-8891, the responsive email corpus with Bates stamps SEC-TITAN-0001001 through 0001007 has been generated in direct Unicode PST format.`,
    attachments: [],
    mapiProperties: [],
    isRead: true,
    isFlagged: true,
    importance: 'high',
    sha256: '2c6ee815338d415b35e77f21a57d72a1e8e2ea98242f3be79645062c93d23e32',
    md5: 'c9f0f895fb98ab9159f51fd0297e236d',
    batesNumber: 'SEC-TITAN-0001008',
    legalHold: true,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_009',
    messageId: '<20261102.33120@titan.corp.local>',
    conversationId: 'conv_sent_03',
    folderId: 'sent',
    subject: 'Migration Schedule Announcement: Cutover Scheduled for Saturday 02:00 UTC',
    from: { name: 'Migration Lead', email: 'adpaolucci@corp.local' },
    to: [{ name: 'All Hands', email: 'all@titan.corp.local' }],
    date: '2026-11-02T09:00:00Z',
    sizeBytes: 1024 * 250,
    bodyText: `All,\n\nThe incremental delta sync completed with zero errors across 45,000 mailboxes. Final delta synchronization will occur this Saturday at 02:00 UTC. Mail flow will switch without interruption.`,
    attachments: [],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'normal',
    sha256: 'fa3583571f07461f031872b471782ab3205b83484f455dd04432e7524ec3d3cf',
    md5: '45c48cce2e2d7fbdea1afc51c7c6ad26',
    batesNumber: 'SEC-TITAN-0001009',
    legalHold: false,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_010',
    messageId: '<20261106.01010@drafts.local>',
    conversationId: 'conv_draft_01',
    folderId: 'drafts',
    subject: '[Draft] Rust libpff FFI Bindings & Direct MAPI Memory Benchmarks',
    from: { name: 'Migration Lead', email: 'adpaolucci@corp.local' },
    to: [{ name: 'Rust Core Team', email: 'rust@corp.local' }],
    date: '2026-11-06T15:00:00Z',
    sizeBytes: 1024 * 320,
    bodyText: `Benchmarking results:\nDirect Rust NDB writer achieves 14,200 msg/sec on NVMe.\nZero memory leaks across 500GB continuous streaming.`,
    attachments: [],
    mapiProperties: [],
    isRead: false,
    isFlagged: false,
    importance: 'normal',
    sha256: '9834876dcfb05cb167a5c24953eba58c4ac89b1adf57f28f2f9d09af107ee9f0',
    md5: 'd3b07384d113edec49eaa6238ad5ff00',
    legalHold: false,
    sourceFormat: 'EML',
  },
  {
    id: 'msg_011',
    messageId: '<20261015.44102@archive.local>',
    conversationId: 'conv_archive_01',
    folderId: 'archive',
    subject: 'Project Titan Seed Funding & Valuation Memorandum',
    from: { name: 'Venture Partners', email: 'invest@venture.com' },
    to: [{ name: 'Founders', email: 'founders@titan.corp.local' }],
    date: '2026-10-15T10:00:00Z',
    sizeBytes: 1024 * 2400, // 2.4 MB
    bodyText: `Term sheet executed at $120M pre-money valuation. Wire transfer reference: WT-902148.\nPreserve in compliance vault for statutory 7-year audit retention.`,
    attachments: [
      {
        id: 'att_006',
        filename: 'Series_A_Executed_Term_Sheet.pdf',
        sizeBytes: 1024 * 2100,
        mimeType: 'application/pdf',
        sha256: '38a6a26df1f52da7b158097b693acb6794828b43ff45cff7743d1a89c836a9cb',
      },
    ],
    mapiProperties: [],
    isRead: true,
    isFlagged: true,
    importance: 'high',
    sha256: '38a6a26df1f52da7b158097b693acb6794828b43ff45cff7743d1a89c836a9cb',
    md5: '6512bd43d9caa6e02c990b0a82652dca',
    batesNumber: 'SEC-TITAN-0001010',
    legalHold: true,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_012',
    messageId: '<20261010.55110@archive.local>',
    conversationId: 'conv_archive_02',
    folderId: 'archive',
    subject: 'Patent Filing: Zero-Outlook High Throughput MAPI Container Serialization',
    from: { name: 'US Patent Office Agent', email: 'agent@ip-firm.com' },
    to: [{ name: 'Chief IP Counsel', email: 'ip@titan.corp.local' }],
    date: '2026-10-10T14:15:00Z',
    sizeBytes: 1024 * 3800, // 3.8 MB
    bodyText: `Application Number US-2026-0918237 granted. Abstract covers pure-Rust generation of Microsoft Personal Storage Table (.pst) containers across Linux, macOS, and Windows.`,
    attachments: [
      {
        id: 'att_007',
        filename: 'Patent_US_2026_0918237_Grant.pdf',
        sizeBytes: 1024 * 3500,
        mimeType: 'application/pdf',
        sha256: '1b854378f773beea0f4a74317f2fe963289a05eb7a3717df20efb191c9535eb0',
      },
    ],
    mapiProperties: [],
    isRead: true,
    isFlagged: false,
    importance: 'high',
    sha256: '1b854378f773beea0f4a74317f2fe963289a05eb7a3717df20efb191c9535eb0',
    md5: 'c20ad4d76fe97759aa27a0c99bff6710',
    batesNumber: 'SEC-TITAN-0001011',
    legalHold: true,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_013',
    messageId: '<20261106.cal01@titan.corp.local>',
    conversationId: 'conv_cal_01',
    folderId: 'calendar',
    subject: 'Global Mail Cutover Go/No-Go Decision Meeting',
    from: { name: 'VP Engineering', email: 'vp@titan.corp.local' },
    to: [{ name: 'Executive Operations', email: 'ops@titan.corp.local' }],
    date: '2026-11-06T18:00:00Z',
    sizeBytes: 1024 * 220,
    bodyText: `Location: Executive War Room 4B / Zoom Bridge\nStart: Nov 7, 2026 01:00 UTC\nEnd: Nov 7, 2026 02:00 UTC\nAgenda: Verify delta checkpoints, review forensic chain of custody certificates, and initiate live DNS cutover.`,
    attachments: [],
    mapiProperties: [
      { tag: '0x001A001F', name: 'PR_MESSAGE_CLASS_W', type: 'PT_UNICODE', value: 'IPM.Schedule.Meeting.Request' },
      { tag: '0x1000001F', name: 'PR_BODY_W', type: 'PT_UNICODE', value: 'Location: Executive War Room 4B...' },
      { tag: '0x0037001F', name: 'PR_SUBJECT_W', type: 'PT_UNICODE', value: 'Global Mail Cutover Go/No-Go Decision Meeting' },
    ],
    isRead: true,
    isFlagged: true,
    importance: 'high',
    sha256: 'a2f913d81b854378f773beea0f4a74317f2fe963289a05eb7a3717df20efb191',
    md5: '98f13708210194c475687be6106a3b84',
    batesNumber: 'SEC-TITAN-0001012',
    legalHold: false,
    sourceFormat: 'PST',
  },
  {
    id: 'msg_014',
    messageId: '<20261106.contact01@titan.corp.local>',
    conversationId: 'conv_contact_01',
    folderId: 'contacts',
    subject: 'Contact Card: Anthony D. Paolucci (mail2mail Creator)',
    from: { name: 'Directory Service', email: 'directory@titan.corp.local' },
    to: [{ name: 'Global Address List', email: 'gal@titan.corp.local' }],
    date: '2026-11-06T12:00:00Z',
    sizeBytes: 1024 * 90,
    bodyText: `Full Name: Anthony D. Paolucci\nTitle: Principal Systems Engineer & Author of mail2mail\nEmail: adpaolucci@gmail.com\nRepository: https://github.com/adpaolucci/mail2mail\nExpertise: Low-level email protocol migration, Rust MAPI NDB, eDiscovery Forensics`,
    attachments: [],
    mapiProperties: [
      { tag: '0x001A001F', name: 'PR_MESSAGE_CLASS_W', type: 'PT_UNICODE', value: 'IPM.Contact' },
      { tag: '0x3A06001F', name: 'PR_GIVEN_NAME_W', type: 'PT_UNICODE', value: 'Anthony' },
      { tag: '0x3A11001F', name: 'PR_SURNAME_W', type: 'PT_UNICODE', value: 'Paolucci' },
      { tag: '0x3A17001F', name: 'PR_TITLE_W', type: 'PT_UNICODE', value: 'Principal Systems Engineer' },
      { tag: '0x39FE001F', name: 'PR_EMAIL_ADDRESS_W', type: 'PT_UNICODE', value: 'adpaolucci@gmail.com' },
    ],
    isRead: true,
    isFlagged: true,
    importance: 'high',
    sha256: '6a5b4c3d2e1f0987654321fedcba0987654321fedcba0987654321fedcba0987',
    md5: '7b8b965ad4bca0e41ab51de7b31363a1',
    batesNumber: 'SEC-TITAN-0001013',
    legalHold: false,
    sourceFormat: 'PST',
  },
];

export interface LegacyFormatInfo {
  id: string;
  name: string;
  extension: string;
  signatureHex: string;
  typicalClient: string;
  description: string;
  era: string;
  conversionMethod: string;
  canExportToPst: boolean;
}

export const LEGACY_FORMATS: LegacyFormatInfo[] = [
  {
    id: 'dbx',
    name: 'Outlook Express DBX',
    extension: '.dbx',
    signatureHex: 'CF AD 12 FE',
    typicalClient: 'Microsoft Outlook Express 5.0 / 6.0',
    description: 'Indexed database store with hierarchical binary b-trees containing raw RFC 822 emails.',
    era: '1998 - 2006 (Windows 98/2000/XP)',
    conversionMethod: 'Zero-Outlook Rust DBX tree walker to MIME and MS-PST NDB injection.',
    canExportToPst: true,
  },
  {
    id: 'nsf',
    name: 'Lotus Notes / IBM Domino NSF',
    extension: '.nsf',
    signatureHex: '1A 00 00 04',
    typicalClient: 'IBM Lotus Notes 6.x - 9.x',
    description: 'Notes Storage Facility with On-Disk Structure (ODS), rich-text CD records, and composite mime items.',
    era: '1989 - Present',
    conversionMethod: 'Native ODS reader mapping CD records to MAPI RTF / HTML and Unicode PST.',
    canExportToPst: true,
  },
  {
    id: 'eml_wlm',
    name: 'Windows Live Mail / Windows Mail',
    extension: '.eml / .edb',
    signatureHex: '46 72 6F 6D 3A ("From:") + ESE DB',
    typicalClient: 'Windows Live Mail 2011/2012',
    description: 'Extensible Storage Engine (ESE / JET Blue) index referencing loose .eml trees.',
    era: '2006 - 2012 (Windows Vista / 7)',
    conversionMethod: 'JET Blue B-Tree crawler reconstructing original folder tree into Unicode PST.',
    canExportToPst: true,
  },
  {
    id: 'sbd_msf',
    name: 'Mozilla Thunderbird SBD / MSF',
    extension: '.msf / .sbd',
    signatureHex: '2F 2F 20 3C 21 2D 2D 20 3C 6D 6F 72 6B',
    typicalClient: 'Thunderbird / Netscape Communicator',
    description: 'Mbox flat files paired with Mork database format index (.msf) and subfolder directories (.sbd).',
    era: '1994 - Present',
    conversionMethod: 'Streaming mboxrd state machine + Mork parser directly to PST.',
    canExportToPst: true,
  },
  {
    id: 'tbb',
    name: 'The Bat! TBB Message Base',
    extension: '.tbb / .qbm',
    signatureHex: '20 20 00 00 54 42 42',
    typicalClient: 'Ritlabs The Bat! v1 - v10',
    description: 'Proprietary sequential message storage format used by European privacy-centric email client.',
    era: '1997 - Present',
    conversionMethod: 'Direct TBB header decode and message boundary parser.',
    canExportToPst: true,
  },
  {
    id: 'mbs',
    name: 'Opera Mail MBS',
    extension: '.mbs',
    signatureHex: '4D 42 53 32',
    typicalClient: 'Opera M2 Mail Client',
    description: 'Variant of Mbox with custom Opera metadata headers and embedded search tokens.',
    era: '2000 - 2013',
    conversionMethod: 'Standardized MIME extraction and MAPI table generation.',
    canExportToPst: true,
  },
  {
    id: 'mbx_eudora',
    name: 'Qualcomm Eudora MBX + TOC',
    extension: '.mbx / .toc',
    signatureHex: '45 75 64 6F 72 61 20 ("Eudora ")',
    typicalClient: 'Eudora 7.1 / Light',
    description: 'Table of Contents binary files (.toc) with byte offsets pointing into raw text .mbx mailboxes.',
    era: '1988 - 2006',
    conversionMethod: 'TOC binary table unpacker with attachment folder re-linking.',
    canExportToPst: true,
  },
  {
    id: 'emlx',
    name: 'Apple Mail EMLX',
    extension: '.emlx',
    signatureHex: '31 30 32 34 0A (Byte count header)',
    typicalClient: 'macOS Mail.app (Mac OS X Tiger to Sequoia)',
    description: 'RFC 822 message preceded by byte count header and followed by an XML or binary Property List (.plist).',
    era: '2005 - Present',
    conversionMethod: 'Byte-slice stripper and plist metadata parser to MAPI PR_MESSAGE_FLAGS.',
    canExportToPst: true,
  },
  {
    id: 'maildir',
    name: 'Unix Maildir / Maildir++',
    extension: 'cur/ , new/ , tmp/',
    signatureHex: 'Filesystem directory structure',
    typicalClient: 'Qmail, Dovecot, Postfix, Courier',
    description: 'One file per email with status flags encoded in file names (e.g. 1730000000.V801I401M1.host:2,S).',
    era: '1995 - Present',
    conversionMethod: 'Directory crawler with POSIX flag translation to MAPI read/flagged bits.',
    canExportToPst: true,
  },
  {
    id: 'ost',
    name: 'Outlook Offline Storage Table (OST)',
    extension: '.ost',
    signatureHex: '21 42 44 4E (!BDN) / 21 42 44 4F (!BDO)',
    typicalClient: 'Microsoft Outlook 2003 - 2024 / M365',
    description: 'Cached Exchange mailbox store; encrypted or orphan OSTs cannot be opened in Outlook without Exchange.',
    era: '2003 - Present',
    conversionMethod: 'Direct NDB B-Tree reader, decrypting Compressible/None, converting to portable PST.',
    canExportToPst: true,
  },
];
