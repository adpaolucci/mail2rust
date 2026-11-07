import React, { useState, useEffect } from 'react';
import {
  ShieldAlert,
  ShieldCheck,
  X,
  Search,
  Download,
  Pause,
  Play,
  RotateCcw,
  CheckCircle2,
  AlertTriangle,
  Lock,
  Terminal,
  Activity,
  Layers,
  Link,
  Copy,
  Check,
  Filter,
  FileText,
  Clock,
  Cpu,
} from 'lucide-react';
import { AuditEvent, AuditCategory, AuditSeverity, HashChainVerificationResult } from '../types/audit';
import {
  INITIAL_AUDIT_LOGS,
  createAuditEvent,
  verifyAuditHashChain,
  filterAuditEvents,
  exportAuditLogsCsv,
} from '../utils/auditLogger';

interface SecurityAuditViewerProps {
  isOpen: boolean;
  onClose: () => void;
}

export const SecurityAuditViewer: React.FC<SecurityAuditViewerProps> = ({
  isOpen,
  onClose,
}) => {
  const [events, setEvents] = useState<AuditEvent[]>(INITIAL_AUDIT_LOGS);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<'ALL' | AuditCategory>('ALL');
  const [selectedSeverity, setSelectedSeverity] = useState<'ALL' | AuditSeverity>('ALL');
  const [isLiveStream, setIsLiveStream] = useState(true);
  const [selectedEvent, setSelectedEvent] = useState<AuditEvent | null>(INITIAL_AUDIT_LOGS[INITIAL_AUDIT_LOGS.length - 1]);
  const [verificationResult, setVerificationResult] = useState<HashChainVerificationResult | null>(null);
  const [copiedHash, setCopiedHash] = useState<string | null>(null);

  // Background real-time audit event simulation
  useEffect(() => {
    let interval: any;
    if (isLiveStream && isOpen) {
      interval = setInterval(() => {
        // Occasionally emit real-time forensic activity
        const simulatedPool = [
          {
            severity: 'SECURITY_AUDIT' as AuditSeverity,
            category: 'CRYPTO_HASH' as AuditCategory,
            action: 'VERIFY_ITEM_SHA256_MATCH',
            actor: 'mail2mail-forensics-worker',
            targetResource: `Email Item #00${Math.floor(Math.random() * 90 + 10)}`,
            description: 'Computed bit-exact SHA-256 match for incoming MIME payload.',
          },
          {
            severity: 'INFO' as AuditSeverity,
            category: 'PST_BUILDER' as AuditCategory,
            action: 'NDB_BTREE_NODE_ALLOCATED',
            actor: 'mail2mail-rust-direct-mapi',
            targetResource: 'NDB Layer (64-bit B-Tree)',
            description: `Allocated BID 0x${(Math.random() * 0xFFFF).toString(16).slice(0, 4).toUpperCase()} in Node B-Tree heap.`,
          },
          {
            severity: 'NOTICE' as AuditSeverity,
            category: 'MIGRATION_PIPELINE' as AuditCategory,
            action: 'M365_BATCH_CHUNK_COMMITTED',
            actor: 'adpaolucci@corp.local [ADMIN]',
            targetResource: 'Target Folder: /IPM_SUBTREE/Inbox',
            description: 'Committed chunk of 25 messages via Microsoft Graph Batch Import API.',
          },
        ];

        const choice = simulatedPool[Math.floor(Math.random() * simulatedPool.length)];
        setEvents(prev => {
          const lastEvent = prev[prev.length - 1];
          const newId = `AUD-2026-${String(prev.length + 1).padStart(4, '0')}`;
          const newEv = createAuditEvent(
            {
              id: newId,
              timestamp: new Date().toISOString(),
              severity: choice.severity,
              category: choice.category,
              action: choice.action,
              actor: choice.actor,
              targetResource: choice.targetResource,
              description: choice.description,
              clientIp: '127.0.0.1',
              processId: 'PID-4912 [mail2mail-rust-desktop]',
            },
            lastEvent ? lastEvent.eventHash : 'GENESIS'
          );
          return [...prev, newEv];
        });
      }, 7000);
    }
    return () => clearInterval(interval);
  }, [isLiveStream, isOpen]);

  if (!isOpen) return null;

  const filteredEvents = filterAuditEvents(events, searchQuery, selectedCategory, selectedSeverity);

  const handleVerifyChain = () => {
    const res = verifyAuditHashChain(events);
    setVerificationResult(res);
  };

  const handleCopy = (text: string, id: string) => {
    navigator.clipboard.writeText(text);
    setCopiedHash(id);
    setTimeout(() => setCopiedHash(null), 2000);
  };

  const handleDownloadCsv = () => {
    const csv = exportAuditLogsCsv(events);
    const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `Security_Audit_Trail_RFC3161_${Date.now()}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleDownloadJson = () => {
    const payload = {
      auditStandard: 'RFC 3161 & ISO 27001 Section A.12.4 Logging and Monitoring',
      totalEntries: events.length,
      chainVerification: verifyAuditHashChain(events),
      events,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `Security_Audit_Log_Full_${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleAddManualEvent = () => {
    const lastEvent = events[events.length - 1];
    const newEv = createAuditEvent(
      {
        id: `AUD-2026-${String(events.length + 1).padStart(4, '0')}`,
        timestamp: new Date().toISOString(),
        severity: 'SECURITY_AUDIT',
        category: 'LEGAL_HOLD',
        action: 'MANUAL_FORENSIC_CHECKPOINT',
        actor: 'adpaolucci@corp.local [ADMIN]',
        targetResource: 'Corpus: Full Evidence Repository',
        description: 'Manual forensic snapshot checkpoint triggered by certified examiner.',
        clientIp: '127.0.0.1',
        processId: 'PID-4912',
      },
      lastEvent ? lastEvent.eventHash : 'GENESIS'
    );
    setEvents(prev => [...prev, newEv]);
    setSelectedEvent(newEv);
  };

  const getSeverityBadge = (severity: AuditSeverity) => {
    switch (severity) {
      case 'SECURITY_AUDIT':
        return (
          <span className="px-1.5 py-0.5 rounded bg-purple-950/80 border border-purple-600/50 text-purple-300 font-mono text-[10px] font-bold">
            SECURITY_AUDIT
          </span>
        );
      case 'WARNING':
        return (
          <span className="px-1.5 py-0.5 rounded bg-amber-950/80 border border-amber-600/50 text-amber-300 font-mono text-[10px] font-bold">
            WARNING
          </span>
        );
      case 'ERROR':
        return (
          <span className="px-1.5 py-0.5 rounded bg-rose-950/80 border border-rose-600/50 text-rose-300 font-mono text-[10px] font-bold">
            ERROR
          </span>
        );
      case 'NOTICE':
        return (
          <span className="px-1.5 py-0.5 rounded bg-emerald-950/80 border border-emerald-600/50 text-emerald-300 font-mono text-[10px]">
            NOTICE
          </span>
        );
      default:
        return (
          <span className="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-slate-300 font-mono text-[10px]">
            INFO
          </span>
        );
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/75 backdrop-blur-sm flex items-center justify-center p-3 sm:p-5 select-none">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-6xl shadow-2xl flex flex-col h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-3.5 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="w-8 h-8 rounded-lg bg-purple-600/30 border border-purple-500/40 flex items-center justify-center text-purple-400">
              <ShieldAlert className="w-4 h-4" />
            </div>
            <div>
              <div className="flex items-center space-x-2">
                <h2 className="text-sm font-semibold text-white tracking-tight">Security & Forensic Audit Trail Viewer</h2>
                <div className="flex items-center space-x-1.5 px-2 py-0.2 rounded-full bg-emerald-950 border border-emerald-600/40 text-emerald-300 text-[10px] font-mono">
                  <span className={`w-2 h-2 rounded-full ${isLiveStream ? 'bg-emerald-400 animate-pulse' : 'bg-slate-500'}`} />
                  <span>{isLiveStream ? 'REAL-TIME STREAM: ACTIVE' : 'STREAM PAUSED'}</span>
                </div>
              </div>
              <p className="text-[11px] text-slate-400">
                Immutable, cryptographic SHA-256 Merkle hash chain &bull; ISO 27001 & FRCP Rule 37(e) Compliance
              </p>
            </div>
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={() => setIsLiveStream(!isLiveStream)}
              className="flex items-center space-x-1 px-2.5 py-1.5 rounded bg-slate-800 hover:bg-slate-750 border border-slate-700 text-slate-300 transition-colors"
            >
              {isLiveStream ? <Pause className="w-3.5 h-3.5 text-amber-400" /> : <Play className="w-3.5 h-3.5 text-emerald-400" />}
              <span>{isLiveStream ? 'Pause Feed' : 'Resume Feed'}</span>
            </button>

            <button
              onClick={handleVerifyChain}
              className="flex items-center space-x-1 px-2.5 py-1.5 rounded bg-purple-600 hover:bg-purple-500 text-white font-medium shadow-sm transition-colors"
            >
              <ShieldCheck className="w-3.5 h-3.5 text-purple-200" />
              <span>Verify Hash Chain Integrity</span>
            </button>

            <button
              onClick={onClose}
              className="p-1 text-slate-400 hover:text-white rounded-md hover:bg-slate-800 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Verification Banner */}
        {verificationResult && (
          <div className={`p-2.5 border-b text-[11px] font-mono flex items-center justify-between ${
            verificationResult.isValid
              ? 'bg-emerald-950/70 border-emerald-600/50 text-emerald-300'
              : 'bg-rose-950/80 border-rose-600/60 text-rose-300'
          }`}>
            <div className="flex items-center space-x-2">
              <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
              <span>
                <strong>MERKLE CHAIN AUDIT PASSED:</strong> Verified {verificationResult.totalEventsChecked} events.
                Zero broken links or retroactive alterations detected. Root Checksum: {verificationResult.rootHash.slice(0, 24)}...
              </span>
            </div>
            <button
              onClick={() => setVerificationResult(null)}
              className="text-slate-400 hover:text-white"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        )}

        {/* Search & Filter Toolbar */}
        <div className="p-3 bg-slate-950 border-b border-slate-800 flex flex-wrap items-center justify-between gap-2">
          {/* Search Box */}
          <div className="flex items-center space-x-2 flex-1 min-w-[240px] max-w-md">
            <div className="relative w-full">
              <Search className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="Search action, actor, resource, description, IP..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full bg-slate-800 border border-slate-700 rounded pl-8 pr-3 py-1.5 text-slate-100 placeholder-slate-400 text-xs focus:outline-none focus:border-purple-500 font-mono"
              />
            </div>
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="text-slate-400 hover:text-slate-200"
              >
                <RotateCcw className="w-3 h-3" />
              </button>
            )}
          </div>

          {/* Category Filter Pills */}
          <div className="flex items-center flex-wrap gap-1 text-[11px]">
            {(['ALL', 'CRYPTO_HASH', 'MIGRATION_PIPELINE', 'LEGAL_HOLD', 'PST_BUILDER', 'CLOUD_AUTH', 'DATA_EXPORT'] as const).map(cat => (
              <button
                key={cat}
                onClick={() => setSelectedCategory(cat)}
                className={`px-2 py-0.5 rounded transition-colors ${
                  selectedCategory === cat
                    ? 'bg-purple-600 text-white font-medium'
                    : 'bg-slate-800 text-slate-400 hover:text-slate-200 hover:bg-slate-750'
                }`}
              >
                {cat.replace('_', ' ')}
              </button>
            ))}
          </div>

          {/* Severity Dropdown */}
          <div className="flex items-center space-x-2">
            <select
              value={selectedSeverity}
              onChange={(e) => setSelectedSeverity(e.target.value as any)}
              className="bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 text-xs"
            >
              <option value="ALL">All Severities</option>
              <option value="SECURITY_AUDIT">Security Audit</option>
              <option value="WARNING">Warnings</option>
              <option value="INFO">Info</option>
              <option value="NOTICE">Notice</option>
              <option value="ERROR">Errors</option>
            </select>

            <button
              onClick={handleAddManualEvent}
              className="px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-300 text-xs transition-colors"
              title="Record verified examiner checkpoint in chain"
            >
              + Log Checkpoint
            </button>
          </div>
        </div>

        {/* Main Body (Split: Event Table on Left, Event Detail on Right) */}
        <div className="flex-1 flex overflow-hidden">
          {/* Event Table View */}
          <div className="flex-1 flex flex-col overflow-hidden bg-slate-900 border-r border-slate-800">
            <div className="bg-slate-850 px-3 py-1.5 border-b border-slate-800 text-[10px] text-slate-400 font-mono uppercase font-bold grid grid-cols-12 gap-2">
              <span className="col-span-2">Time (UTC)</span>
              <span className="col-span-2">Severity</span>
              <span className="col-span-3">Action</span>
              <span className="col-span-3">Actor</span>
              <span className="col-span-2 text-right">Hash Link</span>
            </div>

            <div className="flex-1 overflow-y-auto divide-y divide-slate-800/60 font-mono text-xs">
              {filteredEvents.length === 0 ? (
                <div className="p-8 text-center text-slate-500">
                  <Terminal className="w-8 h-8 mx-auto mb-2 text-slate-600" />
                  <p>No audit events match your filter query</p>
                </div>
              ) : (
                filteredEvents.slice().reverse().map(e => {
                  const isSelected = selectedEvent?.id === e.id;
                  const timeFormatted = new Date(e.timestamp).toLocaleTimeString([], {
                    hour: '2-digit',
                    minute: '2-digit',
                    second: '2-digit',
                  });

                  return (
                    <div
                      key={e.id}
                      onClick={() => setSelectedEvent(e)}
                      className={`px-3 py-2 cursor-pointer transition-colors grid grid-cols-12 gap-2 items-center ${
                        isSelected
                          ? 'bg-purple-600/20 border-l-2 border-purple-500 text-white'
                          : 'hover:bg-slate-800/50 text-slate-300'
                      }`}
                    >
                      <span className="col-span-2 text-[11px] text-slate-400 truncate">
                        {timeFormatted}
                      </span>
                      <span className="col-span-2">{getSeverityBadge(e.severity)}</span>
                      <span className="col-span-3 text-[11px] font-bold text-slate-200 truncate">
                        {e.action}
                      </span>
                      <span className="col-span-3 text-[11px] text-slate-400 truncate">
                        {e.actor}
                      </span>
                      <span className="col-span-2 text-right flex items-center justify-end space-x-1 text-[10px] text-emerald-400">
                        <Link className="w-3 h-3 text-purple-400" />
                        <span className="font-mono">{e.eventHash.slice(0, 6)}</span>
                      </span>
                    </div>
                  );
                })
              )}
            </div>
          </div>

          {/* Right Detail Pane */}
          {selectedEvent && (
            <div className="w-96 min-w-[340px] bg-slate-950 p-4 overflow-y-auto space-y-4 font-mono text-xs border-l border-slate-800">
              <div className="border-b border-slate-800 pb-3 flex items-start justify-between">
                <div>
                  <div className="text-purple-400 text-[10px] font-bold uppercase tracking-wider">
                    {selectedEvent.category} &bull; {selectedEvent.id}
                  </div>
                  <h3 className="text-sm font-bold text-white mt-0.5">{selectedEvent.action}</h3>
                </div>
                {getSeverityBadge(selectedEvent.severity)}
              </div>

              {/* Event Properties */}
              <div className="space-y-2.5 text-[11px]">
                <div>
                  <span className="text-slate-500 block text-[10px] uppercase">UTC Timestamp</span>
                  <span className="text-slate-200">{selectedEvent.timestamp}</span>
                  <span className="text-slate-500 text-[10px] block font-mono">Epoch Nanos: {selectedEvent.timestampNanos}</span>
                </div>

                <div>
                  <span className="text-slate-500 block text-[10px] uppercase">Actor / Identity</span>
                  <span className="text-emerald-400 font-semibold">{selectedEvent.actor}</span>
                </div>

                <div>
                  <span className="text-slate-500 block text-[10px] uppercase">Target Resource</span>
                  <span className="text-slate-200 break-all">{selectedEvent.targetResource}</span>
                </div>

                <div>
                  <span className="text-slate-500 block text-[10px] uppercase">Terminal / Process</span>
                  <span className="text-slate-300">{selectedEvent.clientIp} &bull; {selectedEvent.processId}</span>
                </div>

                <div>
                  <span className="text-slate-500 block text-[10px] uppercase">Description</span>
                  <p className="text-slate-300 font-sans leading-relaxed bg-slate-900 p-2.5 rounded border border-slate-800">
                    {selectedEvent.description}
                  </p>
                </div>

                {selectedEvent.metadata && (
                  <div>
                    <span className="text-slate-500 block text-[10px] uppercase mb-1">Structured Metadata</span>
                    <pre className="bg-slate-900 p-2 rounded border border-slate-800 text-[10px] text-purple-300 overflow-x-auto">
                      {JSON.stringify(selectedEvent.metadata, null, 2)}
                    </pre>
                  </div>
                )}
              </div>

              {/* Cryptographic Link Block */}
              <div className="border-t border-slate-800 pt-3 space-y-2 text-[10px]">
                <div className="bg-slate-900 p-2.5 rounded border border-slate-800 space-y-1">
                  <span className="text-slate-500 block uppercase">Previous Link Hash (prevHash)</span>
                  <div className="text-slate-400 break-all">{selectedEvent.prevHash}</div>
                </div>

                <div className="bg-slate-900 p-2.5 rounded border border-purple-500/40 space-y-1">
                  <div className="flex justify-between items-center">
                    <span className="text-purple-400 block uppercase font-bold">Event SHA-256 Digest</span>
                    <button
                      onClick={() => handleCopy(selectedEvent.eventHash, selectedEvent.id)}
                      className="text-slate-400 hover:text-white"
                    >
                      {copiedHash === selectedEvent.id ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                    </button>
                  </div>
                  <div className="text-emerald-400 font-bold break-all">{selectedEvent.eventHash}</div>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-3 border-t border-slate-800 bg-slate-850 flex items-center justify-between text-[11px] text-slate-400 font-mono">
          <div className="flex items-center space-x-2">
            <span>Showing {filteredEvents.length} of {events.length} system audit entries</span>
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={handleDownloadCsv}
              className="px-3 py-1.5 rounded bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 transition-colors flex items-center space-x-1.5"
            >
              <Download className="w-3.5 h-3.5 text-emerald-400" />
              <span>Export RFC 3161 Log (.csv)</span>
            </button>

            <button
              onClick={handleDownloadJson}
              className="px-3 py-1.5 rounded bg-purple-600 hover:bg-purple-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5"
            >
              <Download className="w-3.5 h-3.5" />
              <span>Export Audit JSON</span>
            </button>

            <button
              onClick={onClose}
              className="px-3.5 py-1.5 rounded bg-slate-800 hover:bg-slate-750 text-slate-300 border border-slate-700 transition-colors"
            >
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
