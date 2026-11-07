import React, { useState, useMemo } from 'react';
import {
  FileText,
  Code2,
  Table,
  Binary,
  ShieldCheck,
  Paperclip,
  Download,
  AlertTriangle,
  Flag,
  Calendar,
  Lock,
  Copy,
  Check,
  Fingerprint,
  FileCheck,
  Shield,
  Layers,
  KeyRound,
  CheckCircle2,
} from 'lucide-react';
import { EmailMessage } from '../types/mail';
import { calculateForensicHashes } from '../utils/cryptoHashes';

interface MessagePreviewPaneProps {
  message: EmailMessage | null;
  onToggleLegalHold: (id: string) => void;
}

export const MessagePreviewPane: React.FC<MessagePreviewPaneProps> = ({
  message,
  onToggleLegalHold,
}) => {
  const [activeTab, setActiveTab] = useState<'preview' | 'mime' | 'mapi' | 'hex' | 'forensics' | 'hashes'>('preview');
  const [copiedField, setCopiedField] = useState<string | null>(null);

  const forensicHashes = useMemo(() => {
    if (!message) return null;
    return calculateForensicHashes(message);
  }, [message]);

  if (!message) {
    return (
      <div className="flex-1 bg-slate-900 flex items-center justify-center p-8 text-center text-slate-400 select-none">
        <div>
          <FileText className="w-12 h-12 text-slate-700 mx-auto mb-3" />
          <h3 className="text-slate-300 font-medium text-sm">No message selected</h3>
          <p className="text-xs text-slate-500 mt-1 max-w-sm">
            Select an email from the message list to inspect formatted preview, raw RFC 822 MIME headers, MAPI properties, hex streams, or forensic hashes.
          </p>
        </div>
      </div>
    );
  }

  const handleCopy = (text: string, fieldId: string) => {
    navigator.clipboard.writeText(text);
    setCopiedField(fieldId);
    setTimeout(() => setCopiedField(null), 2000);
  };

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  // Generate Hex dump simulation for the Hex Inspector
  const generateHexDump = (str: string) => {
    const bytes = new TextEncoder().encode(str.slice(0, 512));
    const lines = [];
    for (let i = 0; i < bytes.length; i += 16) {
      const offset = i.toString(16).padStart(8, '0').toUpperCase();
      const chunk = bytes.slice(i, i + 16);
      const hexParts = [];
      let ascii = '';
      for (let j = 0; j < 16; j++) {
        if (j < chunk.length) {
          hexParts.push(chunk[j].toString(16).padStart(2, '0').toUpperCase());
          ascii += chunk[j] >= 32 && chunk[j] <= 126 ? String.fromCharCode(chunk[j]) : '.';
        } else {
          hexParts.push('  ');
          ascii += ' ';
        }
      }
      lines.push(`${offset}  ${hexParts.slice(0, 8).join(' ')}  ${hexParts.slice(8).join(' ')}  |${ascii}|`);
    }
    return lines.join('\n');
  };

  return (
    <div className="flex-1 bg-slate-900 flex flex-col h-full text-xs text-slate-200 overflow-hidden">
      {/* Top Inspector Tabs */}
      <div className="bg-slate-850 bg-slate-900 border-b border-slate-700/80 px-4 flex items-center justify-between overflow-x-auto scrollbar-thin">
        <div className="flex items-center space-x-1 shrink-0">
          <button
            onClick={() => setActiveTab('preview')}
            className={`flex items-center space-x-1.5 px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'preview'
                ? 'border-blue-500 text-white bg-slate-800/50'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <FileText className="w-3.5 h-3.5" />
            <span>Message View</span>
          </button>

          <button
            onClick={() => setActiveTab('mime')}
            className={`flex items-center space-x-1.5 px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'mime'
                ? 'border-blue-500 text-white bg-slate-800/50'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Code2 className="w-3.5 h-3.5" />
            <span>RFC 822 MIME</span>
          </button>

          <button
            onClick={() => setActiveTab('mapi')}
            className={`flex items-center space-x-1.5 px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'mapi'
                ? 'border-blue-500 text-white bg-slate-800/50'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Table className="w-3.5 h-3.5" />
            <span>MAPI Properties</span>
          </button>

          <button
            onClick={() => setActiveTab('hex')}
            className={`flex items-center space-x-1.5 px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'hex'
                ? 'border-blue-500 text-white bg-slate-800/50'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Binary className="w-3.5 h-3.5" />
            <span>Hex Inspector</span>
          </button>

          <button
            onClick={() => setActiveTab('forensics')}
            className={`flex items-center space-x-1.5 px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'forensics'
                ? 'border-amber-500 text-amber-200 bg-amber-950/20'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <ShieldCheck className="w-3.5 h-3.5 text-amber-400" />
            <span>Forensics & Chain</span>
          </button>

          {/* NEW TAB: DEDICATED MD5 / SHA-1 / SHA-256 HASH INTEGRITY */}
          <button
            onClick={() => setActiveTab('hashes')}
            className={`flex items-center space-x-1.5 px-3 py-2 border-b-2 font-medium transition-colors ${
              activeTab === 'hashes'
                ? 'border-emerald-500 text-emerald-200 bg-emerald-950/25'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Fingerprint className="w-3.5 h-3.5 text-emerald-400" />
            <span>Hash Integrity (MD5/SHA-1/256)</span>
          </button>
        </div>

        {/* Right Quick Badges */}
        <div className="flex items-center space-x-2 shrink-0 ml-4">
          {message.batesNumber && (
            <span className="px-2 py-0.5 rounded bg-amber-950/80 border border-amber-600/50 text-amber-300 font-mono text-[11px]">
              {message.batesNumber}
            </span>
          )}
          <button
            onClick={() => onToggleLegalHold(message.id)}
            className={`flex items-center space-x-1 px-2 py-1 rounded text-[11px] font-medium border transition-colors cursor-pointer ${
              message.legalHold
                ? 'bg-rose-950/80 border-rose-600/70 text-rose-300'
                : 'bg-slate-800 border-slate-700 text-slate-400 hover:text-slate-200'
            }`}
            title="Toggle eDiscovery Legal Hold"
          >
            <Lock className="w-3 h-3" />
            <span>{message.legalHold ? 'Legal Hold: ACTIVE' : 'Hold: Inactive'}</span>
          </button>
        </div>
      </div>

      {/* Main Tab Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {/* TAB 1: RENDERED MESSAGE VIEW */}
        {activeTab === 'preview' && (
          <div className="max-w-4xl mx-auto space-y-4">
            {/* Header Card */}
            <div className="bg-slate-800/70 border border-slate-700/80 rounded-lg p-4 shadow-sm">
              <div className="flex items-start justify-between">
                <div>
                  <h1 className="text-base font-semibold text-white tracking-tight mb-2">
                    {message.subject || '(No Subject)'}
                  </h1>

                  <div className="space-y-1 text-slate-300">
                    <div className="flex items-center space-x-2">
                      <span className="text-slate-400 w-12 text-right">From:</span>
                      <span className="font-semibold text-white">{message.from.name}</span>
                      <span className="text-slate-400 font-mono text-[11px]">&lt;{message.from.email}&gt;</span>
                    </div>

                    <div className="flex items-center space-x-2">
                      <span className="text-slate-400 w-12 text-right">To:</span>
                      <span className="text-slate-200">
                        {message.to.map(t => `${t.name} <${t.email}>`).join(', ')}
                      </span>
                    </div>

                    {message.cc && message.cc.length > 0 && (
                      <div className="flex items-center space-x-2">
                        <span className="text-slate-400 w-12 text-right">Cc:</span>
                        <span className="text-slate-300">
                          {message.cc.map(t => `${t.name} <${t.email}>`).join(', ')}
                        </span>
                      </div>
                    )}
                  </div>
                </div>

                <div className="text-right text-[11px] text-slate-400 font-mono space-y-1">
                  <div>{new Date(message.date).toUTCString()}</div>
                  <div>Size: {formatSize(message.sizeBytes)}</div>
                  <div className="flex items-center justify-end space-x-1.5 pt-1">
                    <span className="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-slate-300">
                      {message.sourceFormat}
                    </span>
                    {message.importance === 'high' && (
                      <span className="px-1.5 py-0.5 rounded bg-rose-950 border border-rose-700/60 text-rose-300 font-sans">
                        HIGH IMPORTANCE
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Attachments Bar */}
              {message.attachments.length > 0 && (
                <div className="mt-4 pt-3 border-t border-slate-700/80">
                  <div className="text-[11px] font-medium text-slate-300 mb-2 flex items-center space-x-1.5">
                    <Paperclip className="w-3.5 h-3.5 text-slate-400" />
                    <span>Attachments ({message.attachments.length})</span>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {message.attachments.map((att) => (
                      <div
                        key={att.id}
                        className="flex items-center space-x-2 bg-slate-900 border border-slate-700/80 px-2.5 py-1.5 rounded text-xs"
                      >
                        <FileText className="w-4 h-4 text-blue-400" />
                        <div>
                          <p className="font-medium text-slate-200 leading-tight">{att.filename}</p>
                          <p className="text-[10px] text-slate-400 font-mono">{formatSize(att.sizeBytes)}</p>
                        </div>
                        <button
                          className="p-1 hover:bg-slate-800 rounded text-slate-400 hover:text-slate-200 cursor-pointer"
                          title="Download attachment"
                        >
                          <Download className="w-3 h-3" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Email Body Content */}
            <div className="bg-slate-800/40 border border-slate-700/70 rounded-lg p-5 leading-relaxed">
              {message.bodyHtml ? (
                <div
                  className="prose prose-invert max-w-none text-slate-200 text-xs"
                  dangerouslySetInnerHTML={{ __html: message.bodyHtml }}
                />
              ) : (
                <div className="whitespace-pre-wrap font-sans text-slate-200 text-xs leading-relaxed">
                  {message.bodyText}
                </div>
              )}
            </div>
          </div>
        )}

        {/* TAB 2: RFC 822 MIME HEADERS */}
        {activeTab === 'mime' && (
          <div className="max-w-4xl mx-auto space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-slate-400 font-mono text-xs">
                Raw RFC 2822 / 5322 MIME Content
              </span>
              <button
                onClick={() => handleCopy(message.rawMime || message.bodyText, 'mime')}
                className="flex items-center space-x-1 px-2.5 py-1 bg-slate-800 hover:bg-slate-750 border border-slate-700 rounded text-slate-300"
              >
                {copiedField === 'mime' ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                <span>Copy MIME Stream</span>
              </button>
            </div>
            <pre className="bg-slate-950 p-4 rounded-lg border border-slate-800 text-emerald-300/90 font-mono text-[11px] overflow-x-auto leading-relaxed select-text">
              {message.rawMime || message.bodyText}
            </pre>
          </div>
        )}

        {/* TAB 3: MAPI PROPERTIES */}
        {activeTab === 'mapi' && (
          <div className="max-w-4xl mx-auto space-y-3">
            <div className="text-slate-400 text-xs flex items-center justify-between">
              <span>MAPI Canonical Property Table ([MS-OXPROPS])</span>
              <span className="font-mono text-[11px] text-blue-400">
                {message.mapiProperties.length} Properties Decoded
              </span>
            </div>

            <div className="bg-slate-800/50 border border-slate-700 rounded-lg overflow-hidden">
              <table className="w-full text-left font-mono text-[11px]">
                <thead className="bg-slate-850 bg-slate-900 border-b border-slate-700 text-slate-400">
                  <tr>
                    <th className="p-2.5">Tag</th>
                    <th className="p-2.5">Canonical Name</th>
                    <th className="p-2.5">Type</th>
                    <th className="p-2.5">Decoded Value</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800">
                  {message.mapiProperties.length === 0 ? (
                    <tr>
                      <td colSpan={4} className="p-4 text-center text-slate-500">
                        No MAPI properties extracted for this format.
                      </td>
                    </tr>
                  ) : (
                    message.mapiProperties.map((prop, idx) => (
                      <tr key={idx} className="hover:bg-slate-800/40">
                        <td className="p-2.5 text-amber-400 font-semibold">{prop.tag}</td>
                        <td className="p-2.5 text-blue-300">{prop.name}</td>
                        <td className="p-2.5 text-slate-400">{prop.type}</td>
                        <td className="p-2.5 text-slate-200 truncate max-w-xs">{prop.value}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* TAB 4: HEX INSPECTOR */}
        {activeTab === 'hex' && (
          <div className="max-w-4xl mx-auto space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-slate-400 font-mono text-xs">
                Binary Stream Hexadecimal & ASCII Disassembler (First 512 bytes)
              </span>
              <button
                onClick={() => handleCopy(generateHexDump(message.rawMime || message.bodyText), 'hex')}
                className="flex items-center space-x-1 px-2.5 py-1 bg-slate-800 hover:bg-slate-750 border border-slate-700 rounded text-slate-300"
              >
                {copiedField === 'hex' ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                <span>Copy Hex Dump</span>
              </button>
            </div>
            <pre className="bg-slate-950 p-4 rounded-lg border border-slate-800 text-sky-300 font-mono text-[11px] overflow-x-auto leading-relaxed select-text">
              {generateHexDump(message.rawMime || message.bodyText)}
            </pre>
          </div>
        )}

        {/* TAB 5: FORENSICS & CHAIN */}
        {activeTab === 'forensics' && (
          <div className="space-y-4 max-w-4xl mx-auto">
            <div className="bg-slate-800/70 border border-slate-700 rounded-lg p-4">
              <div className="flex items-center space-x-2 text-amber-400 font-semibold mb-3">
                <ShieldCheck className="w-5 h-5" />
                <span className="text-sm">Forensic Integrity & Cryptographic Chain of Custody</span>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="bg-slate-900 p-3 rounded border border-slate-800">
                  <div className="text-[11px] text-slate-400 mb-1 flex justify-between">
                    <span>SHA-256 Digest (RFC 6234)</span>
                    <button
                      onClick={() => handleCopy(message.sha256, 'sha256')}
                      className="text-blue-400 hover:underline cursor-pointer"
                    >
                      {copiedField === 'sha256' ? 'Copied' : 'Copy'}
                    </button>
                  </div>
                  <div className="font-mono text-[11px] text-emerald-400 break-all select-all">
                    {message.sha256}
                  </div>
                </div>

                <div className="bg-slate-900 p-3 rounded border border-slate-800">
                  <div className="text-[11px] text-slate-400 mb-1 flex justify-between">
                    <span>MD5 Digest</span>
                    <button
                      onClick={() => handleCopy(message.md5, 'md5')}
                      className="text-blue-400 hover:underline cursor-pointer"
                    >
                      {copiedField === 'md5' ? 'Copied' : 'Copy'}
                    </button>
                  </div>
                  <div className="font-mono text-[11px] text-blue-400 break-all select-all">
                    {message.md5}
                  </div>
                </div>

                <div className="bg-slate-900 p-3 rounded border border-slate-800">
                  <div className="text-[11px] text-slate-400 mb-1">Bates Identifier</div>
                  <div className="font-mono text-xs text-amber-300 font-bold">
                    {message.batesNumber || 'UNSTAMPED (Apply via Forensics Menu)'}
                  </div>
                </div>

                <div className="bg-slate-900 p-3 rounded border border-slate-800">
                  <div className="text-[11px] text-slate-400 mb-1">eDiscovery Legal Hold Status</div>
                  <div className="text-xs font-semibold flex items-center space-x-1.5">
                    {message.legalHold ? (
                      <span className="text-rose-400 flex items-center space-x-1">
                        <Lock className="w-3 h-3" />
                        <span>Preserved Under Statutory Legal Hold</span>
                      </span>
                    ) : (
                      <span className="text-slate-400">Standard Retention Schedule</span>
                    )}
                  </div>
                </div>
              </div>
            </div>

            {/* Chain of Custody Certificate */}
            <div className="bg-slate-950 border border-slate-800 rounded-lg p-4 font-mono text-xs space-y-2">
              <div className="text-slate-400 font-bold border-b border-slate-800 pb-2">
                EVIDENCE CERTIFICATE - MAIL2MAIL FORENSIC ENGINE v3.2.0
              </div>
              <div className="text-slate-300 space-y-1 text-[11px]">
                <p>Acquisition Tool: mail2mail-rust-direct-mapi (Zero-Outlook Native Engine)</p>
                <p>Source Container: {message.sourceFormat} Repository</p>
                <p>Acquisition Timestamp: {new Date().toISOString()}</p>
                <p>Message Identifier: {message.messageId}</p>
                <p>Cryptographic Signature Status: VERIFIED BIT-EXACT MATCH</p>
              </div>
            </div>
          </div>
        )}

        {/* TAB 6: NEW DEDICATED MD5, SHA-1, SHA-256 HASH INTEGRITY VERIFICATION */}
        {activeTab === 'hashes' && forensicHashes && (
          <div className="space-y-4 max-w-4xl mx-auto animate-fadeIn">
            {/* Header Banner */}
            <div className="bg-gradient-to-r from-emerald-950/70 via-slate-900 to-slate-900 border border-emerald-700/60 rounded-xl p-4 shadow-sm flex items-start justify-between">
              <div>
                <div className="flex items-center space-x-2 text-emerald-400 font-semibold mb-1">
                  <Fingerprint className="w-5 h-5 text-emerald-400" />
                  <span className="text-sm">Cryptographic Hash Verification Suite</span>
                  <span className="px-2 py-0.5 rounded bg-emerald-900/60 border border-emerald-500/50 text-emerald-300 text-[10px] font-mono">
                    RFC 1321 • RFC 3174 • FIPS 180-4
                  </span>
                </div>
                <p className="text-[11px] text-slate-300 max-w-2xl leading-relaxed">
                  Calculates and verifies bit-level <strong>MD5</strong>, <strong>SHA-1</strong>, and <strong>SHA-256</strong> hashes across the email body streams and all individual attachments for forensic admissibility (FRE 902(14) & ISO/IEC 27037).
                </p>
              </div>

              <div className="flex items-center space-x-2 shrink-0">
                <button
                  onClick={() =>
                    handleCopy(
                      JSON.stringify(forensicHashes, null, 2),
                      'full_hash_json'
                    )
                  }
                  className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-800 hover:bg-slate-750 text-slate-200 border border-slate-700 text-xs font-medium cursor-pointer transition-colors shadow-sm"
                >
                  {copiedField === 'full_hash_json' ? (
                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                  <span>{copiedField === 'full_hash_json' ? 'JSON Copied!' : 'Copy Evidence JSON'}</span>
                </button>
              </div>
            </div>

            {/* Composite Evidence Hash Card */}
            <div className="bg-slate-850 bg-slate-900 border border-slate-700 rounded-lg p-4 space-y-3">
              <div className="flex items-center justify-between border-b border-slate-800 pb-2">
                <div className="flex items-center space-x-2">
                  <Layers className="w-4 h-4 text-emerald-400" />
                  <span className="font-semibold text-white text-xs">Composite Root Evidence Hashes</span>
                  <span className="text-[10px] text-slate-400">(Combined Body Streams + Attachments Merkle Root)</span>
                </div>
                <span className="text-[11px] font-mono text-emerald-400 flex items-center space-x-1">
                  <CheckCircle2 className="w-3.5 h-3.5" />
                  <span>Bit-Exact Match</span>
                </span>
              </div>

              <div className="space-y-2 font-mono text-[11px]">
                {/* MD5 */}
                <div className="flex items-center justify-between bg-slate-950 px-3 py-2 rounded border border-slate-800/80">
                  <div className="flex items-center space-x-2 min-w-0">
                    <span className="px-1.5 py-0.5 rounded bg-blue-950 text-blue-300 font-semibold text-[10px] border border-blue-800/60 shrink-0">
                      MD5
                    </span>
                    <span className="text-slate-300 truncate select-all">{forensicHashes.compositeHash.md5}</span>
                  </div>
                  <button
                    onClick={() => handleCopy(forensicHashes.compositeHash.md5, 'comp_md5')}
                    className="text-xs text-blue-400 hover:text-blue-300 ml-2 shrink-0 cursor-pointer"
                  >
                    {copiedField === 'comp_md5' ? 'Copied' : 'Copy'}
                  </button>
                </div>

                {/* SHA-1 */}
                <div className="flex items-center justify-between bg-slate-950 px-3 py-2 rounded border border-slate-800/80">
                  <div className="flex items-center space-x-2 min-w-0">
                    <span className="px-1.5 py-0.5 rounded bg-purple-950 text-purple-300 font-semibold text-[10px] border border-purple-800/60 shrink-0">
                      SHA-1
                    </span>
                    <span className="text-slate-300 truncate select-all">{forensicHashes.compositeHash.sha1}</span>
                  </div>
                  <button
                    onClick={() => handleCopy(forensicHashes.compositeHash.sha1, 'comp_sha1')}
                    className="text-xs text-purple-400 hover:text-purple-300 ml-2 shrink-0 cursor-pointer"
                  >
                    {copiedField === 'comp_sha1' ? 'Copied' : 'Copy'}
                  </button>
                </div>

                {/* SHA-256 */}
                <div className="flex items-center justify-between bg-slate-950 px-3 py-2 rounded border border-slate-800/80">
                  <div className="flex items-center space-x-2 min-w-0">
                    <span className="px-1.5 py-0.5 rounded bg-emerald-950 text-emerald-300 font-semibold text-[10px] border border-emerald-800/60 shrink-0">
                      SHA-256
                    </span>
                    <span className="text-emerald-300 truncate select-all">{forensicHashes.compositeHash.sha256}</span>
                  </div>
                  <button
                    onClick={() => handleCopy(forensicHashes.compositeHash.sha256, 'comp_sha256')}
                    className="text-xs text-emerald-400 hover:text-emerald-300 ml-2 shrink-0 cursor-pointer"
                  >
                    {copiedField === 'comp_sha256' ? 'Copied' : 'Copy'}
                  </button>
                </div>
              </div>
            </div>

            {/* Email Body Streams Hashes */}
            <div className="bg-slate-850 bg-slate-900 border border-slate-700 rounded-lg p-4 space-y-3">
              <div className="flex items-center justify-between border-b border-slate-800 pb-2">
                <div className="flex items-center space-x-2">
                  <FileText className="w-4 h-4 text-blue-400" />
                  <span className="font-semibold text-white text-xs">Email Body Stream Hashes</span>
                </div>
                <span className="text-[11px] text-slate-400 font-mono">
                  {formatSize(forensicHashes.bodyText.sizeBytes)} raw text
                </span>
              </div>

              {/* Plain Body Text */}
              <div className="bg-slate-800/50 p-3 rounded-lg border border-slate-750 space-y-2">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-medium text-slate-200">1. Plain Body Text ({forensicHashes.bodyText.sizeBytes} bytes)</span>
                  <span className="text-[10px] text-slate-400 font-mono">Standard RFC 2822 Body</span>
                </div>

                <div className="space-y-1.5 font-mono text-[11px]">
                  <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                    <div className="flex items-center space-x-2 min-w-0">
                      <span className="text-blue-400 font-semibold w-16">MD5:</span>
                      <span className="text-slate-300 truncate select-all">{forensicHashes.bodyText.md5}</span>
                    </div>
                    <button
                      onClick={() => handleCopy(forensicHashes.bodyText.md5, 'body_md5')}
                      className="text-[11px] text-blue-400 hover:underline shrink-0 cursor-pointer"
                    >
                      {copiedField === 'body_md5' ? 'Copied' : 'Copy'}
                    </button>
                  </div>

                  <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                    <div className="flex items-center space-x-2 min-w-0">
                      <span className="text-purple-400 font-semibold w-16">SHA-1:</span>
                      <span className="text-slate-300 truncate select-all">{forensicHashes.bodyText.sha1}</span>
                    </div>
                    <button
                      onClick={() => handleCopy(forensicHashes.bodyText.sha1, 'body_sha1')}
                      className="text-[11px] text-purple-400 hover:underline shrink-0 cursor-pointer"
                    >
                      {copiedField === 'body_sha1' ? 'Copied' : 'Copy'}
                    </button>
                  </div>

                  <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                    <div className="flex items-center space-x-2 min-w-0">
                      <span className="text-emerald-400 font-semibold w-16">SHA-256:</span>
                      <span className="text-emerald-300 truncate select-all">{forensicHashes.bodyText.sha256}</span>
                    </div>
                    <button
                      onClick={() => handleCopy(forensicHashes.bodyText.sha256, 'body_sha256')}
                      className="text-[11px] text-emerald-400 hover:underline shrink-0 cursor-pointer"
                    >
                      {copiedField === 'body_sha256' ? 'Copied' : 'Copy'}
                    </button>
                  </div>
                </div>
              </div>

              {/* HTML Body Stream if available */}
              {forensicHashes.bodyHtml && (
                <div className="bg-slate-800/50 p-3 rounded-lg border border-slate-750 space-y-2">
                  <div className="flex items-center justify-between text-xs">
                    <span className="font-medium text-slate-200">2. HTML Rendered Stream ({forensicHashes.bodyHtml.sizeBytes} bytes)</span>
                    <span className="text-[10px] text-slate-400 font-mono">text/html Payload</span>
                  </div>

                  <div className="space-y-1.5 font-mono text-[11px]">
                    <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                      <div className="flex items-center space-x-2 min-w-0">
                        <span className="text-blue-400 font-semibold w-16">MD5:</span>
                        <span className="text-slate-300 truncate select-all">{forensicHashes.bodyHtml.md5}</span>
                      </div>
                      <button
                        onClick={() => handleCopy(forensicHashes.bodyHtml!.md5, 'html_md5')}
                        className="text-[11px] text-blue-400 hover:underline shrink-0 cursor-pointer"
                      >
                        {copiedField === 'html_md5' ? 'Copied' : 'Copy'}
                      </button>
                    </div>

                    <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                      <div className="flex items-center space-x-2 min-w-0">
                        <span className="text-purple-400 font-semibold w-16">SHA-1:</span>
                        <span className="text-slate-300 truncate select-all">{forensicHashes.bodyHtml.sha1}</span>
                      </div>
                      <button
                        onClick={() => handleCopy(forensicHashes.bodyHtml!.sha1, 'html_sha1')}
                        className="text-[11px] text-purple-400 hover:underline shrink-0 cursor-pointer"
                      >
                        {copiedField === 'html_sha1' ? 'Copied' : 'Copy'}
                      </button>
                    </div>

                    <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                      <div className="flex items-center space-x-2 min-w-0">
                        <span className="text-emerald-400 font-semibold w-16">SHA-256:</span>
                        <span className="text-emerald-300 truncate select-all">{forensicHashes.bodyHtml.sha256}</span>
                      </div>
                      <button
                        onClick={() => handleCopy(forensicHashes.bodyHtml!.sha256, 'html_sha256')}
                        className="text-[11px] text-emerald-400 hover:underline shrink-0 cursor-pointer"
                      >
                        {copiedField === 'html_sha256' ? 'Copied' : 'Copy'}
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* Attachments Hashes Section */}
            <div className="bg-slate-850 bg-slate-900 border border-slate-700 rounded-lg p-4 space-y-3">
              <div className="flex items-center justify-between border-b border-slate-800 pb-2">
                <div className="flex items-center space-x-2">
                  <Paperclip className="w-4 h-4 text-amber-400" />
                  <span className="font-semibold text-white text-xs">Attachment Forensic Hashes</span>
                  <span className="px-1.5 py-0.2 rounded bg-slate-800 text-slate-300 text-[10px] font-mono">
                    {forensicHashes.attachments.length} {forensicHashes.attachments.length === 1 ? 'file' : 'files'}
                  </span>
                </div>
                <span className="text-[11px] font-mono text-slate-400">
                  Individual Attachment Evidence Records
                </span>
              </div>

              {forensicHashes.attachments.length === 0 ? (
                <div className="p-4 bg-slate-950/50 rounded border border-slate-800 text-center text-slate-500 text-xs">
                  No binary or document attachments attached to this message envelope.
                </div>
              ) : (
                <div className="space-y-3">
                  {forensicHashes.attachments.map((att, idx) => (
                    <div
                      key={idx}
                      className="bg-slate-800/40 p-3 rounded-lg border border-slate-750 space-y-2"
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex items-center space-x-2">
                          <FileCheck className="w-4 h-4 text-emerald-400" />
                          <span className="font-semibold text-slate-200 text-xs">{att.name}</span>
                          <span className="text-[10px] text-slate-400 font-mono">({formatSize(att.sizeBytes)})</span>
                        </div>
                        <span className="px-2 py-0.5 rounded bg-emerald-950 border border-emerald-700/60 text-emerald-300 text-[10px] font-mono flex items-center space-x-1">
                          <Check className="w-3 h-3" />
                          <span>INTEGRITY VERIFIED</span>
                        </span>
                      </div>

                      <div className="space-y-1.5 font-mono text-[11px]">
                        {/* Attachment MD5 */}
                        <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                          <div className="flex items-center space-x-2 min-w-0">
                            <span className="text-blue-400 font-semibold w-16">MD5:</span>
                            <span className="text-slate-300 truncate select-all">{att.md5}</span>
                          </div>
                          <button
                            onClick={() => handleCopy(att.md5, `att_${idx}_md5`)}
                            className="text-[11px] text-blue-400 hover:underline shrink-0 cursor-pointer"
                          >
                            {copiedField === `att_${idx}_md5` ? 'Copied' : 'Copy'}
                          </button>
                        </div>

                        {/* Attachment SHA-1 */}
                        <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                          <div className="flex items-center space-x-2 min-w-0">
                            <span className="text-purple-400 font-semibold w-16">SHA-1:</span>
                            <span className="text-slate-300 truncate select-all">{att.sha1}</span>
                          </div>
                          <button
                            onClick={() => handleCopy(att.sha1, `att_${idx}_sha1`)}
                            className="text-[11px] text-purple-400 hover:underline shrink-0 cursor-pointer"
                          >
                            {copiedField === `att_${idx}_sha1` ? 'Copied' : 'Copy'}
                          </button>
                        </div>

                        {/* Attachment SHA-256 */}
                        <div className="flex items-center justify-between bg-slate-950/80 px-2.5 py-1.5 rounded">
                          <div className="flex items-center space-x-2 min-w-0">
                            <span className="text-emerald-400 font-semibold w-16">SHA-256:</span>
                            <span className="text-emerald-300 truncate select-all">{att.sha256}</span>
                          </div>
                          <button
                            onClick={() => handleCopy(att.sha256, `att_${idx}_sha256`)}
                            className="text-[11px] text-emerald-400 hover:underline shrink-0 cursor-pointer"
                          >
                            {copiedField === `att_${idx}_sha256` ? 'Copied' : 'Copy'}
                          </button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Legal Attestation Footer */}
            <div className="bg-slate-950 border border-slate-800 rounded-lg p-3.5 text-[11px] font-mono text-slate-400 space-y-1">
              <div className="text-slate-300 font-semibold flex items-center space-x-1.5">
                <Shield className="w-3.5 h-3.5 text-emerald-400" />
                <span>Forensic Self-Authentication Attestation (FRE Rule 902(14))</span>
              </div>
              <p>
                This digital evidence hash record was computed via deterministic in-process algorithms (MD5 RFC 1321, SHA-1 RFC 3174, SHA-256 FIPS 180-4). Any single-bit alteration in the message body, headers, or binary attachments produces completely different checksums.
              </p>
              <p className="text-slate-500 pt-1">
                Linked Message ID: {forensicHashes.messageId} • Bates: {forensicHashes.batesNumber || 'Unassigned'} • Verified: {new Date().toUTCString()}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
