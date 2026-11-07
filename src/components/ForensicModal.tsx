import React, { useState } from 'react';
import {
  ShieldAlert,
  X,
  FileText,
  CheckCircle,
  Hash,
  Download,
  Lock,
  Layers,
  Copy,
  Check,
} from 'lucide-react';
import { EmailMessage, ForensicManifest } from '../types/mail';
import { applyBatesStamping, deduplicateMessages, generateForensicManifest } from '../utils/forensics';

interface ForensicModalProps {
  isOpen: boolean;
  onClose: () => void;
  messages: EmailMessage[];
  onUpdateMessages: (updated: EmailMessage[]) => void;
}

export const ForensicModal: React.FC<ForensicModalProps> = ({
  isOpen,
  onClose,
  messages,
  onUpdateMessages,
}) => {
  const [batesPrefix, setBatesPrefix] = useState('LEGAL-TITAN');
  const [batesStartIndex, setBatesStartIndex] = useState(1001);
  const [caseNumber, setCaseNumber] = useState('26-CV-8891-DISC');
  const [examinerName, setExaminerName] = useState('Anthony D. Paolucci (Lead Forensic Examiner)');
  const [activeTab, setActiveTab] = useState<'bates' | 'dedup' | 'manifest'>('bates');
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleApplyBates = () => {
    const stamped = applyBatesStamping(messages, batesPrefix, batesStartIndex);
    onUpdateMessages(stamped);
    setStatusMessage(`Successfully stamped ${stamped.length} messages with range ${batesPrefix}-${String(batesStartIndex).padStart(7, '0')} to ${batesPrefix}-${String(batesStartIndex + stamped.length - 1).padStart(7, '0')}.`);
  };

  const handleDeduplicate = (strategy: 'sha256' | 'messageId' | 'strictBody') => {
    const { unique, duplicates } = deduplicateMessages(messages, strategy);
    onUpdateMessages(unique);
    setStatusMessage(`Deduplication complete: retained ${unique.length} unique messages, purged ${duplicates.length} duplicate items.`);
  };

  const manifest: ForensicManifest = generateForensicManifest(
    messages,
    caseNumber,
    examinerName,
    'Enterprise PST / Cloud Mailbox Corpus',
    'Independent MS-PST Container'
  );

  const handleExportManifestJson = () => {
    const fullForensicPackage = {
      manifest,
      evidenceItems: messages.map(m => ({
        batesNumber: m.batesNumber || 'N/A',
        subject: m.subject,
        sender: m.from.email,
        recipients: m.to.map(t => t.email),
        date: m.date,
        sha256: m.sha256,
        md5: m.md5,
        sizeBytes: m.sizeBytes,
        legalHold: m.legalHold,
        attachments: m.attachments.map(a => ({
          filename: a.filename,
          sizeBytes: a.sizeBytes,
          sha256: a.sha256,
        })),
      })),
    };

    const blob = new Blob([JSON.stringify(fullForensicPackage, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `Forensic_Chain_Of_Custody_${caseNumber}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-3xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-amber-600/30 border border-amber-500/40 flex items-center justify-center text-amber-400">
              <ShieldAlert className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Forensic, Legal & eDiscovery Suite</h2>
              <p className="text-[11px] text-slate-400">
                Cryptographic hashing, Bates stamping, chain of custody certificates & de-duplication
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 text-slate-400 hover:text-white rounded-md hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="bg-slate-950 border-b border-slate-800 px-4 flex items-center space-x-2">
          <button
            onClick={() => setActiveTab('bates')}
            className={`px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'bates'
                ? 'border-amber-500 text-amber-400 bg-slate-900'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Bates Stamping Sequencer
          </button>
          <button
            onClick={() => setActiveTab('dedup')}
            className={`px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'dedup'
                ? 'border-amber-500 text-amber-400 bg-slate-900'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Hash De-Duplication
          </button>
          <button
            onClick={() => setActiveTab('manifest')}
            className={`px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'manifest'
                ? 'border-amber-500 text-amber-400 bg-slate-900'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Chain of Custody Manifest
          </button>
        </div>

        {/* Body */}
        <div className="p-5 overflow-y-auto space-y-4">
          {statusMessage && (
            <div className="bg-emerald-950/70 border border-emerald-600/50 rounded-lg p-3 text-emerald-300 text-[11px] flex items-center space-x-2">
              <CheckCircle className="w-4 h-4 text-emerald-400 shrink-0" />
              <span>{statusMessage}</span>
            </div>
          )}

          {/* TAB 1: BATES STAMPING */}
          {activeTab === 'bates' && (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-slate-300 font-medium mb-1">Bates Identifier Prefix</label>
                  <input
                    type="text"
                    value={batesPrefix}
                    onChange={(e) => setBatesPrefix(e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                  />
                </div>
                <div>
                  <label className="block text-slate-300 font-medium mb-1">Starting Sequence Number</label>
                  <input
                    type="number"
                    value={batesStartIndex}
                    onChange={(e) => setBatesStartIndex(Number(e.target.value))}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                  />
                </div>
              </div>

              {/* Sample Bates Preview */}
              <div className="bg-slate-950 border border-slate-800 rounded-lg p-3.5 space-y-2">
                <span className="font-semibold text-slate-300">Bates Stamping Range Preview</span>
                <div className="grid grid-cols-2 gap-2 text-xs font-mono">
                  <div className="bg-slate-900 p-2 rounded border border-slate-800">
                    <span className="text-slate-500 text-[10px] block">First Document Stamp</span>
                    <span className="text-amber-300 font-bold">
                      {batesPrefix}-{String(batesStartIndex).padStart(7, '0')}
                    </span>
                  </div>
                  <div className="bg-slate-900 p-2 rounded border border-slate-800">
                    <span className="text-slate-500 text-[10px] block">Last Document Stamp</span>
                    <span className="text-amber-300 font-bold">
                      {batesPrefix}-{String(batesStartIndex + messages.length - 1).padStart(7, '0')}
                    </span>
                  </div>
                </div>
              </div>

              <button
                onClick={handleApplyBates}
                className="w-full py-2 px-4 rounded-lg bg-amber-600 hover:bg-amber-500 text-white font-medium flex items-center justify-center space-x-2 transition-colors shadow-sm"
              >
                <Hash className="w-4 h-4" />
                <span>Stamp {messages.length} Messages with Bates Sequence</span>
              </button>
            </div>
          )}

          {/* TAB 2: DE-DUPLICATION */}
          {activeTab === 'dedup' && (
            <div className="space-y-4">
              <p className="text-slate-400 text-[11px] leading-relaxed">
                Remove identical or near-duplicate messages across mailboxes to reduce eDiscovery review review costs and storage footprint.
              </p>

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                <button
                  onClick={() => handleDeduplicate('sha256')}
                  className="p-3 bg-slate-800 hover:bg-slate-750 border border-slate-700 rounded-lg text-left transition-colors"
                >
                  <span className="font-semibold text-white block mb-1">Strict SHA-256 Hash</span>
                  <span className="text-[11px] text-slate-400 block">
                    Removes exact bit-for-bit duplicate payloads with identical hashes.
                  </span>
                </button>

                <button
                  onClick={() => handleDeduplicate('messageId')}
                  className="p-3 bg-slate-800 hover:bg-slate-750 border border-slate-700 rounded-lg text-left transition-colors"
                >
                  <span className="font-semibold text-white block mb-1">RFC 822 Message-ID</span>
                  <span className="text-[11px] text-slate-400 block">
                    Deduplicates cross-folder copies sharing the same Internet Message-ID.
                  </span>
                </button>

                <button
                  onClick={() => handleDeduplicate('strictBody')}
                  className="p-3 bg-slate-800 hover:bg-slate-750 border border-slate-700 rounded-lg text-left transition-colors"
                >
                  <span className="font-semibold text-white block mb-1">Body & Sender Hash</span>
                  <span className="text-[11px] text-slate-400 block">
                    Normalizes whitespace and matches subject + body content.
                  </span>
                </button>
              </div>
            </div>
          )}

          {/* TAB 3: CHAIN OF CUSTODY MANIFEST */}
          {activeTab === 'manifest' && (
            <div className="space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-slate-400 text-[11px] mb-1">Case / Matter Number</label>
                  <input
                    type="text"
                    value={caseNumber}
                    onChange={(e) => setCaseNumber(e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                  />
                </div>
                <div>
                  <label className="block text-slate-400 text-[11px] mb-1">Certified Forensic Examiner</label>
                  <input
                    type="text"
                    value={examinerName}
                    onChange={(e) => setExaminerName(e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 text-[11px]"
                  />
                </div>
              </div>

              {/* Manifest Output Card */}
              <div className="bg-slate-950 border border-slate-800 rounded-lg p-4 font-mono text-[11px] space-y-1.5 text-slate-300">
                <div className="text-amber-400 font-bold border-b border-slate-800 pb-1 mb-2">
                  IMMUTABLE CHAIN OF CUSTODY AUDIT LOG
                </div>
                <div>Root Corpus SHA-256: <span className="text-emerald-400 break-all">{manifest.rootSha256}</span></div>
                <div>Total Evidence Items: <span className="text-blue-400">{manifest.totalItems}</span></div>
                <div>Total Volume Size: <span className="text-blue-400">{(manifest.totalSizeBytes / (1024 * 1024)).toFixed(2)} MB</span></div>
                <div>Acquisition Timestamp: <span className="text-slate-400">{manifest.timestamp}</span></div>
                <div>Engine Specification: <span className="text-slate-400">{manifest.toolVersion}</span></div>
              </div>

              <button
                onClick={handleExportManifestJson}
                className="w-full py-2 px-4 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium flex items-center justify-center space-x-2 transition-colors shadow-sm"
              >
                <Download className="w-4 h-4" />
                <span>Export Forensic Chain of Custody Package (.json)</span>
              </button>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <span className="text-[11px] text-slate-400 font-mono">
            {messages.length} items in evidence store
          </span>

          <button
            onClick={onClose}
            className="px-4 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
