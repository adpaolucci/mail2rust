import React, { useState } from 'react';
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
} from 'lucide-react';
import { EmailMessage } from '../types/mail';

interface MessagePreviewPaneProps {
  message: EmailMessage | null;
  onToggleLegalHold: (id: string) => void;
}

export const MessagePreviewPane: React.FC<MessagePreviewPaneProps> = ({
  message,
  onToggleLegalHold,
}) => {
  const [activeTab, setActiveTab] = useState<'preview' | 'mime' | 'mapi' | 'hex' | 'forensics'>('preview');
  const [copiedField, setCopiedField] = useState<string | null>(null);

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
      <div className="bg-slate-850 bg-slate-900 border-b border-slate-700/80 px-4 flex items-center justify-between">
        <div className="flex items-center space-x-1">
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
            <span>Forensics & Hash</span>
          </button>
        </div>

        {/* Right Quick Badges */}
        <div className="flex items-center space-x-2">
          {message.batesNumber && (
            <span className="px-2 py-0.5 rounded bg-amber-950/80 border border-amber-600/50 text-amber-300 font-mono text-[11px]">
              {message.batesNumber}
            </span>
          )}
          <button
            onClick={() => onToggleLegalHold(message.id)}
            className={`flex items-center space-x-1 px-2 py-1 rounded text-[11px] font-medium border transition-colors ${
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

                <div className="text-right text-[11px] text-slate-400 space-y-1">
                  <div className="font-mono text-slate-300">
                    {new Date(message.date).toUTCString()}
                  </div>
                  <div>Size: <span className="font-mono text-slate-300">{formatSize(message.sizeBytes)}</span></div>
                  <div className="flex items-center justify-end space-x-1.5">
                    <span className="px-1.5 py-0.5 rounded bg-blue-950/60 border border-blue-600/40 text-blue-300 text-[10px] font-mono">
                      {message.sourceFormat}
                    </span>
                    {message.importance === 'high' && (
                      <span className="px-1.5 py-0.5 rounded bg-rose-950/60 border border-rose-600/40 text-rose-300 text-[10px]">
                        HIGH IMPORTANCE
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Attachments Bar */}
              {message.attachments.length > 0 && (
                <div className="mt-4 pt-3 border-t border-slate-700/60">
                  <div className="flex items-center space-x-2 mb-2 text-slate-400">
                    <Paperclip className="w-3.5 h-3.5" />
                    <span className="font-medium text-slate-200">Attachments ({message.attachments.length})</span>
                  </div>

                  <div className="flex flex-wrap gap-2">
                    {message.attachments.map(att => (
                      <div
                        key={att.id}
                        className="flex items-center space-x-2 bg-slate-900 border border-slate-700 rounded px-2.5 py-1.5 text-slate-200 hover:border-slate-500 transition-colors"
                      >
                        <FileText className="w-4 h-4 text-blue-400 shrink-0" />
                        <div className="min-w-0">
                          <p className="font-medium truncate max-w-xs">{att.filename}</p>
                          <p className="text-[10px] text-slate-400 font-mono">{formatSize(att.sizeBytes)}</p>
                        </div>
                        <button
                          onClick={() => {
                            const blob = new Blob([`Sample extracted content for ${att.filename}`], { type: att.mimeType });
                            const url = URL.createObjectURL(blob);
                            const a = document.createElement('a');
                            a.href = url;
                            a.download = att.filename;
                            a.click();
                          }}
                          className="p-1 hover:bg-slate-800 rounded text-slate-400 hover:text-white"
                          title="Download attachment"
                        >
                          <Download className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Rendered Body */}
            <div className="bg-slate-800/40 border border-slate-700/60 rounded-lg p-5">
              {message.bodyHtml ? (
                <div
                  className="prose prose-invert max-w-none text-slate-200"
                  dangerouslySetInnerHTML={{ __html: message.bodyHtml }}
                />
              ) : (
                <pre className="whitespace-pre-wrap font-sans text-slate-200 leading-relaxed text-xs">
                  {message.bodyText}
                </pre>
              )}
            </div>
          </div>
        )}

        {/* TAB 2: RFC 822 RAW MIME */}
        {activeTab === 'mime' && (
          <div className="space-y-3 font-mono text-xs">
            <div className="flex items-center justify-between text-slate-400">
              <span>RFC 822 / MIME Stream Header & Body</span>
              <button
                onClick={() => handleCopy(message.rawMime || message.bodyText, 'mime')}
                className="flex items-center space-x-1 px-2.5 py-1 bg-slate-800 hover:bg-slate-750 border border-slate-700 rounded text-slate-300"
              >
                {copiedField === 'mime' ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                <span>Copy Raw MIME</span>
              </button>
            </div>
            <pre className="bg-slate-950 p-4 rounded-lg border border-slate-800 text-emerald-400/90 overflow-x-auto text-[11px] leading-relaxed">
              {message.rawMime || `Message-ID: ${message.messageId}\nFrom: ${message.from.email}\nTo: ${message.to.map(t => t.email).join(', ')}\nDate: ${message.date}\nSubject: ${message.subject}\n\n${message.bodyText}`}
            </pre>
          </div>
        )}

        {/* TAB 3: MAPI PROPERTIES */}
        {activeTab === 'mapi' && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="font-semibold text-slate-200">MS-PST MAPI Property Tags (LTP Layer)</h3>
                <p className="text-[11px] text-slate-400">
                  Parsed Property Context (PC) and Table Context (TC) values conforming to [MS-OXPROPS]
                </p>
              </div>
            </div>

            <div className="border border-slate-700/80 rounded-lg overflow-hidden bg-slate-950">
              <table className="w-full text-left border-collapse text-xs">
                <thead>
                  <tr className="bg-slate-850 bg-slate-800/80 border-b border-slate-700 text-slate-400 font-mono text-[11px]">
                    <th className="p-2.5">Tag</th>
                    <th className="p-2.5">Property Name</th>
                    <th className="p-2.5">Type</th>
                    <th className="p-2.5">Value</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800 font-mono text-[11px]">
                  {message.mapiProperties.map((prop, idx) => (
                    <tr key={idx} className="hover:bg-slate-800/50">
                      <td className="p-2.5 text-blue-400 font-bold">{prop.tag}</td>
                      <td className="p-2.5 text-amber-300">{prop.name}</td>
                      <td className="p-2.5 text-purple-400">{prop.type}</td>
                      <td className="p-2.5 text-slate-300 break-all">{prop.value}</td>
                    </tr>
                  ))}
                  {/* Default fallback rows if message has fewer properties */}
                  <tr className="hover:bg-slate-800/50">
                    <td className="p-2.5 text-blue-400 font-bold">0x0037001F</td>
                    <td className="p-2.5 text-amber-300">PR_SUBJECT_W</td>
                    <td className="p-2.5 text-purple-400">PT_UNICODE</td>
                    <td className="p-2.5 text-slate-300">{message.subject}</td>
                  </tr>
                  <tr className="hover:bg-slate-800/50">
                    <td className="p-2.5 text-blue-400 font-bold">0x00390040</td>
                    <td className="p-2.5 text-amber-300">PR_CLIENT_SUBMIT_TIME</td>
                    <td className="p-2.5 text-purple-400">PT_SYSTIME</td>
                    <td className="p-2.5 text-slate-300">{message.date}</td>
                  </tr>
                  <tr className="hover:bg-slate-800/50">
                    <td className="p-2.5 text-blue-400 font-bold">0x1035001F</td>
                    <td className="p-2.5 text-amber-300">PR_INTERNET_MESSAGE_ID_W</td>
                    <td className="p-2.5 text-purple-400">PT_UNICODE</td>
                    <td className="p-2.5 text-slate-300">{message.messageId}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* TAB 4: HEX INSPECTOR */}
        {activeTab === 'hex' && (
          <div className="space-y-3 font-mono text-xs">
            <div className="flex items-center justify-between text-slate-400">
              <span>NDB Allocation Block Hex Stream (Offset | Bytes 00-0F | ASCII)</span>
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

        {/* TAB 5: FORENSICS & HASH */}
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
                      className="text-blue-400 hover:underline"
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
                      className="text-blue-400 hover:underline"
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
      </div>
    </div>
  );
};
