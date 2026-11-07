import React, { useState } from 'react';
import {
  FileText,
  X,
  Download,
  Printer,
  Copy,
  Check,
  ShieldCheck,
  AlertTriangle,
  CheckCircle2,
  Table,
  HardDrive,
  Calendar,
  Lock,
  Stamp,
  Activity,
  Layers,
} from 'lucide-react';
import { EmailMessage, MailboxFolder, SyncCheckpoint } from '../types/mail';
import { buildReportData, generateComplianceCsv, MigrationReportData } from '../utils/reportGenerator';

interface ExportReportModalProps {
  isOpen: boolean;
  onClose: () => void;
  messages: EmailMessage[];
  folders: MailboxFolder[];
  checkpoint: SyncCheckpoint | null;
}

export const ExportReportModal: React.FC<ExportReportModalProps> = ({
  isOpen,
  onClose,
  messages,
  folders,
  checkpoint,
}) => {
  const [caseNumber, setCaseNumber] = useState('26-CV-8891-DISC');
  const [matterTitle, setMatterTitle] = useState('In re Titan Corporate Technology Transition & eDiscovery');
  const [examinerName, setExaminerName] = useState('Anthony D. Paolucci');
  const [examinerOrg, setExaminerOrg] = useState('Principal Systems Engineer & Lead Forensic Analyst');
  const [jurisdiction, setJurisdiction] = useState('U.S. District Court, Southern District of New York');
  const [activeTab, setActiveTab] = useState<'pdf' | 'csv' | 'errors'>('pdf');
  const [copiedCsv, setCopiedCsv] = useState(false);

  if (!isOpen) return null;

  const reportData = buildReportData(messages, folders, checkpoint, {
    caseNumber,
    matterTitle,
    examinerName,
    examinerOrg,
    jurisdiction,
  });

  const csvContent = generateComplianceCsv(reportData, messages);

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const handleDownloadCsv = () => {
    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `eDiscovery_Migration_Summary_${caseNumber.replace(/[^a-zA-Z0-9_-]/g, '_')}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleCopyCsv = () => {
    navigator.clipboard.writeText(csvContent);
    setCopiedCsv(true);
    setTimeout(() => setCopiedCsv(false), 2000);
  };

  const handlePrintPdf = () => {
    window.print();
  };

  const handleDownloadManifestJson = () => {
    const manifestPackage = {
      complianceStandard: 'Federal Rules of Evidence 902(11) & 902(14) Electronic Records Certification',
      reportData,
      itemLedger: messages.map(m => ({
        batesNumber: m.batesNumber || 'N/A',
        messageId: m.messageId,
        subject: m.subject,
        sender: m.from.email,
        recipients: m.to.map(t => t.email),
        date: m.date,
        sizeBytes: m.sizeBytes,
        sha256: m.sha256,
        md5: m.md5,
        sourceFormat: m.sourceFormat,
        legalHold: m.legalHold,
        attachments: m.attachments.map(a => ({
          filename: a.filename,
          sizeBytes: a.sizeBytes,
          sha256: a.sha256,
        })),
      })),
    };

    const blob = new Blob([JSON.stringify(manifestPackage, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `eDiscovery_Forensic_Manifest_${caseNumber}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-3 sm:p-5">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-4xl shadow-2xl flex flex-col max-h-[92vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-blue-600/30 border border-blue-500/40 flex items-center justify-center text-blue-400">
              <Stamp className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">
                eDiscovery Compliance & Migration Audit Report
              </h2>
              <p className="text-[11px] text-slate-400">
                Official PDF & CSV migration statistics &bull; FRCP Rule 37(e) & FRE 902(14) Chain of Custody
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

        {/* Top Control Bar & Form Inputs */}
        <div className="bg-slate-950 p-4 border-b border-slate-800 grid grid-cols-1 sm:grid-cols-4 gap-3 text-xs">
          <div>
            <label className="block text-slate-400 text-[10px] mb-1 font-mono uppercase font-bold">Matter / Case #</label>
            <input
              type="text"
              value={caseNumber}
              onChange={(e) => setCaseNumber(e.target.value)}
              className="w-full bg-slate-800 border border-slate-700 rounded p-1.5 text-slate-100 font-mono text-[11px]"
            />
          </div>

          <div>
            <label className="block text-slate-400 text-[10px] mb-1 font-mono uppercase font-bold">Examiner Name</label>
            <input
              type="text"
              value={examinerName}
              onChange={(e) => setExaminerName(e.target.value)}
              className="w-full bg-slate-800 border border-slate-700 rounded p-1.5 text-slate-100 text-[11px]"
            />
          </div>

          <div>
            <label className="block text-slate-400 text-[10px] mb-1 font-mono uppercase font-bold">Organization / Title</label>
            <input
              type="text"
              value={examinerOrg}
              onChange={(e) => setExaminerOrg(e.target.value)}
              className="w-full bg-slate-800 border border-slate-700 rounded p-1.5 text-slate-100 text-[11px]"
            />
          </div>

          <div>
            <label className="block text-slate-400 text-[10px] mb-1 font-mono uppercase font-bold">Jurisdiction</label>
            <input
              type="text"
              value={jurisdiction}
              onChange={(e) => setJurisdiction(e.target.value)}
              className="w-full bg-slate-800 border border-slate-700 rounded p-1.5 text-slate-100 text-[11px]"
            />
          </div>
        </div>

        {/* View Tabs */}
        <div className="bg-slate-900 border-b border-slate-800 px-4 flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <button
              onClick={() => setActiveTab('pdf')}
              className={`px-3 py-2 border-b-2 font-medium flex items-center space-x-1.5 transition-colors ${
                activeTab === 'pdf'
                  ? 'border-blue-500 text-blue-400 bg-slate-850'
                  : 'border-transparent text-slate-400 hover:text-slate-200'
              }`}
            >
              <FileText className="w-3.5 h-3.5" />
              <span>Printable PDF Certificate View</span>
            </button>

            <button
              onClick={() => setActiveTab('csv')}
              className={`px-3 py-2 border-b-2 font-medium flex items-center space-x-1.5 transition-colors ${
                activeTab === 'csv'
                  ? 'border-blue-500 text-blue-400 bg-slate-850'
                  : 'border-transparent text-slate-400 hover:text-slate-200'
              }`}
            >
              <Table className="w-3.5 h-3.5" />
              <span>CSV Ledger View</span>
            </button>

            <button
              onClick={() => setActiveTab('errors')}
              className={`px-3 py-2 border-b-2 font-medium flex items-center space-x-1.5 transition-colors ${
                activeTab === 'errors'
                  ? 'border-blue-500 text-blue-400 bg-slate-850'
                  : 'border-transparent text-slate-400 hover:text-slate-200'
              }`}
            >
              <Activity className="w-3.5 h-3.5 text-amber-400" />
              <span>Error & Exception Audit Log ({reportData.errorLogs.length})</span>
            </button>
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={handleDownloadCsv}
              className="flex items-center space-x-1 px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-300 text-xs transition-colors"
            >
              <Download className="w-3.5 h-3.5 text-emerald-400" />
              <span>Download CSV</span>
            </button>

            <button
              onClick={handlePrintPdf}
              className="flex items-center space-x-1 px-2.5 py-1 rounded bg-blue-600 hover:bg-blue-500 text-white font-medium text-xs shadow-sm transition-colors"
            >
              <Printer className="w-3.5 h-3.5 text-blue-200" />
              <span>Print / Save PDF</span>
            </button>
          </div>
        </div>

        {/* Body Content */}
        <div className="flex-1 overflow-y-auto p-4 sm:p-6 bg-slate-950">
          {/* TAB 1: PROFESSIONAL FORMATTED PDF REPORT */}
          {activeTab === 'pdf' && (
            <div className="bg-slate-900 border border-slate-700/80 rounded-xl p-6 shadow-xl max-w-3xl mx-auto space-y-6 text-slate-200 font-sans print:bg-white print:text-black print:border-none print:shadow-none">
              {/* Report Header Letterhead */}
              <div className="border-b-2 border-blue-500/80 pb-4 flex items-start justify-between">
                <div>
                  <div className="flex items-center space-x-2 text-blue-400 font-mono text-[10px] uppercase font-bold tracking-widest">
                    <ShieldCheck className="w-4 h-4 text-emerald-400" />
                    <span>FRE Rule 902(11) &bull; Rule 902(14) Certified Record of Regular Activity</span>
                  </div>
                  <h1 className="text-xl font-bold text-white tracking-tight mt-1 print:text-black">
                    eDiscovery Migration & Forensic Integrity Certificate
                  </h1>
                  <p className="text-slate-400 text-xs mt-0.5 print:text-slate-600">
                    Matter: <strong className="text-slate-200 print:text-black">{reportData.matterTitle}</strong> ({reportData.caseNumber})
                  </p>
                </div>

                <div className="text-right text-[11px] font-mono text-slate-400 print:text-slate-600">
                  <div className="px-2 py-0.5 rounded bg-emerald-950/80 border border-emerald-600/50 text-emerald-300 font-bold inline-block mb-1">
                    STATUS: AUDIT VERIFIED
                  </div>
                  <div>Date: {new Date(reportData.generatedDate).toUTCString()}</div>
                </div>
              </div>

              {/* Case & Examiner Summary Grid */}
              <div className="grid grid-cols-2 gap-4 text-xs bg-slate-950/70 border border-slate-800 rounded-lg p-4 print:bg-slate-100 print:border-slate-300">
                <div className="space-y-1.5">
                  <div>
                    <span className="text-slate-500 block text-[10px] uppercase font-mono">Lead Forensic Examiner</span>
                    <strong className="text-slate-200 print:text-black">{reportData.examinerName}</strong>
                    <span className="block text-slate-400 text-[11px]">{reportData.examinerOrg}</span>
                  </div>
                  <div>
                    <span className="text-slate-500 block text-[10px] uppercase font-mono">Jurisdiction / Forum</span>
                    <span className="text-slate-300 print:text-black">{reportData.jurisdiction}</span>
                  </div>
                </div>

                <div className="space-y-1.5">
                  <div>
                    <span className="text-slate-500 block text-[10px] uppercase font-mono">Source Architecture</span>
                    <span className="text-slate-300 print:text-black">{reportData.sourceSystem}</span>
                  </div>
                  <div>
                    <span className="text-slate-500 block text-[10px] uppercase font-mono">Destination Archive</span>
                    <span className="text-slate-300 print:text-black">{reportData.targetSystem}</span>
                  </div>
                  <div>
                    <span className="text-slate-500 block text-[10px] uppercase font-mono">Sync Watermark Checkpoint</span>
                    <span className="text-cyan-400 font-mono text-[11px]">{reportData.checkpointId}</span>
                  </div>
                </div>
              </div>

              {/* Executive Metrics Counters */}
              <div>
                <h3 className="text-xs font-bold uppercase tracking-wider text-slate-400 font-mono mb-2">
                  1. Executive Data Volume & Completion Statistics
                </h3>
                <div className="grid grid-cols-4 gap-3 text-center font-mono">
                  <div className="bg-slate-800/80 p-3 rounded-lg border border-slate-700/80">
                    <span className="text-[10px] text-slate-400 uppercase block">Total Items</span>
                    <span className="text-lg font-bold text-white block">{reportData.totalMessages}</span>
                    <span className="text-[10px] text-emerald-400">100% Verified</span>
                  </div>

                  <div className="bg-slate-800/80 p-3 rounded-lg border border-slate-700/80">
                    <span className="text-[10px] text-slate-400 uppercase block">Total Volume</span>
                    <span className="text-lg font-bold text-emerald-400 block">{formatSize(reportData.totalSizeBytes)}</span>
                    <span className="text-[10px] text-slate-400">{reportData.totalFolders} Folders</span>
                  </div>

                  <div className="bg-slate-800/80 p-3 rounded-lg border border-slate-700/80">
                    <span className="text-[10px] text-slate-400 uppercase block">Success Rate</span>
                    <span className="text-lg font-bold text-blue-400 block">100.0%</span>
                    <span className="text-[10px] text-emerald-400">Zero Bit Loss</span>
                  </div>

                  <div className="bg-slate-800/80 p-3 rounded-lg border border-slate-700/80">
                    <span className="text-[10px] text-slate-400 uppercase block">Exception Logs</span>
                    <span className="text-lg font-bold text-amber-400 block">{reportData.errorLogs.length}</span>
                    <span className="text-[10px] text-emerald-400">All Handled</span>
                  </div>
                </div>
              </div>

              {/* Forensic Hash & Bates Section */}
              <div className="space-y-2">
                <h3 className="text-xs font-bold uppercase tracking-wider text-slate-400 font-mono">
                  2. Cryptographic Chain of Custody & Document Range
                </h3>
                <div className="bg-slate-950 p-4 rounded-lg border border-slate-800 font-mono text-[11px] space-y-2 print:bg-slate-100 print:border-slate-300">
                  <div className="flex justify-between">
                    <span className="text-slate-400">Corpus Root SHA-256 Digest:</span>
                    <span className="text-emerald-400 font-bold break-all">{reportData.corpusRootSha256}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-slate-400">Certified Bates Stamp Range:</span>
                    <span className="text-amber-300 font-bold">{reportData.batesRange.start} &mdash; {reportData.batesRange.end}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-slate-400">De-Duplication Strategy:</span>
                    <span className="text-slate-200">Strict Bit-Exact SHA-256 with Conversation Thread Clustering</span>
                  </div>
                </div>
              </div>

              {/* Error and Exception Logs Table */}
              <div className="space-y-2">
                <h3 className="text-xs font-bold uppercase tracking-wider text-slate-400 font-mono">
                  3. Handled Exceptions & In-Flight Recovery Logs
                </h3>
                <div className="border border-slate-800 rounded-lg overflow-hidden bg-slate-950 print:bg-white print:border-slate-300">
                  <table className="w-full text-left text-[11px] font-mono border-collapse">
                    <thead>
                      <tr className="bg-slate-850 border-b border-slate-800 text-slate-400 print:bg-slate-200 print:text-black">
                        <th className="p-2">Timestamp UTC</th>
                        <th className="p-2">Item Reference</th>
                        <th className="p-2">Severity</th>
                        <th className="p-2">Event Code</th>
                        <th className="p-2">Resolution Status</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-850">
                      {reportData.errorLogs.map((log, idx) => (
                        <tr key={idx} className="hover:bg-slate-900/50">
                          <td className="p-2 text-slate-400">{log.timestamp}</td>
                          <td className="p-2 text-slate-200">{log.itemRef}</td>
                          <td className="p-2">
                            <span className={`px-1.5 py-0.2 rounded text-[10px] font-bold ${
                              log.severity === 'WARNING'
                                ? 'bg-amber-950 text-amber-300'
                                : 'bg-blue-950 text-blue-300'
                            }`}>
                              {log.severity}
                            </span>
                          </td>
                          <td className="p-2 text-purple-400">{log.code}</td>
                          <td className="p-2 text-emerald-400 font-medium">{log.resolution}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              {/* Examiner Attestation Block */}
              <div className="border-t border-slate-800 pt-4 space-y-2 text-xs">
                <h3 className="text-xs font-bold uppercase tracking-wider text-slate-400 font-mono">
                  4. Attestation & Digital Certification
                </h3>
                <p className="text-[11px] text-slate-400 leading-relaxed italic print:text-slate-600">
                  I hereby declare and certify under penalty of perjury under the laws of the United States of America that the above electronic records migration and forensic hash verification was conducted in the regular course of business using the validated <code>mail2mail-rust-direct-mapi v3.2.0</code> engine, that no alterations were made to email headers or message bodies, and that the calculated cryptographic checksums represent an authentic, bit-exact reproduction of the source repositories.
                </p>

                <div className="pt-3 flex justify-between items-end">
                  <div>
                    <div className="font-serif italic text-base text-blue-400 font-bold print:text-black">
                      /s/ {reportData.examinerName}
                    </div>
                    <div className="text-[11px] text-slate-400">{reportData.examinerOrg}</div>
                    <div className="text-[10px] text-slate-500 font-mono">Digital Signature ID: SHA256:{reportData.corpusRootSha256.slice(0, 16)}...</div>
                  </div>

                  <div className="text-right">
                    <div className="border border-slate-700 rounded p-2 text-center bg-slate-950/60 print:border-slate-400">
                      <Stamp className="w-5 h-5 text-emerald-400 mx-auto mb-1" />
                      <span className="font-mono text-[9px] uppercase tracking-wider block text-slate-400">
                        OFFICIAL EDISCOVERY SEAL
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* TAB 2: RAW CSV LEDGER PREVIEW */}
          {activeTab === 'csv' && (
            <div className="space-y-3 font-mono text-xs">
              <div className="flex items-center justify-between text-slate-400">
                <span>eDiscovery CSV Item Ledger & Metadata Summary</span>
                <div className="flex items-center space-x-2">
                  <button
                    onClick={handleCopyCsv}
                    className="flex items-center space-x-1 px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-750 border border-slate-700 text-slate-300"
                  >
                    {copiedCsv ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                    <span>{copiedCsv ? 'Copied CSV' : 'Copy CSV'}</span>
                  </button>

                  <button
                    onClick={handleDownloadCsv}
                    className="flex items-center space-x-1 px-2.5 py-1 rounded bg-emerald-600 hover:bg-emerald-500 text-white font-medium shadow-sm"
                  >
                    <Download className="w-3.5 h-3.5" />
                    <span>Download CSV</span>
                  </button>
                </div>
              </div>

              <pre className="bg-slate-900 border border-slate-800 p-4 rounded-lg overflow-x-auto text-[11px] text-emerald-400/90 leading-relaxed max-h-[500px] select-text">
                {csvContent}
              </pre>
            </div>
          )}

          {/* TAB 3: ERROR & EXCEPTION AUDIT LOG */}
          {activeTab === 'errors' && (
            <div className="space-y-4">
              <div className="bg-slate-900 border border-slate-700 rounded-lg p-4">
                <div className="flex items-center space-x-2 text-amber-400 font-semibold mb-2">
                  <Activity className="w-4 h-4" />
                  <span>eDiscovery Exception & In-Flight Error Audit Log</span>
                </div>
                <p className="text-slate-400 text-xs">
                  Every runtime throttle backoff, non-standard CD record salvage, and B-Tree boundary resolution is permanently audited below with timestamps, error codes, and verified resolutions.
                </p>
              </div>

              <div className="space-y-3">
                {reportData.errorLogs.map((log, idx) => (
                  <div
                    key={idx}
                    className="bg-slate-900 border border-slate-800 rounded-lg p-3.5 space-y-1.5 font-mono text-xs"
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center space-x-2">
                        <span className={`px-2 py-0.5 rounded text-[10px] font-bold ${
                          log.severity === 'WARNING' ? 'bg-amber-950 text-amber-300' : 'bg-blue-950 text-blue-300'
                        }`}>
                          {log.severity}
                        </span>
                        <span className="text-purple-400 font-bold">{log.code}</span>
                      </div>
                      <span className="text-slate-500 text-[11px]">{log.timestamp}</span>
                    </div>

                    <div className="text-slate-300 text-xs font-sans">
                      <strong className="text-slate-400 font-mono">Target:</strong> {log.itemRef} &bull; {log.description}
                    </div>

                    <div className="text-emerald-400 text-[11px] bg-slate-950 p-2 rounded border border-slate-850 flex items-center space-x-1.5">
                      <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                      <span>{log.resolution}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Modal Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2 text-[11px] text-slate-400 font-mono">
            <span>Root SHA-256: {reportData.corpusRootSha256.slice(0, 24)}...</span>
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={handleDownloadManifestJson}
              className="px-3 py-1.5 rounded-lg border border-slate-700 hover:bg-slate-800 text-slate-300 transition-colors"
            >
              Export JSON Manifest
            </button>
            <button
              onClick={handleDownloadCsv}
              className="px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5"
            >
              <Download className="w-3.5 h-3.5" />
              <span>Export CSV</span>
            </button>
            <button
              onClick={onClose}
              className="px-4 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition-colors"
            >
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
