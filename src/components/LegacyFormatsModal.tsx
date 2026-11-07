import React, { useState } from 'react';
import {
  Layers,
  X,
  Search,
  CheckCircle,
  Binary,
  ArrowRight,
  HardDrive,
  Cpu,
  FileCode,
} from 'lucide-react';
import { LEGACY_FORMATS, LegacyFormatInfo } from '../data/seedData';
import { EmailMessage } from '../types/mail';

interface LegacyFormatsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onImportSampleLegacy: (msg: EmailMessage) => void;
}

export const LegacyFormatsModal: React.FC<LegacyFormatsModalProps> = ({
  isOpen,
  onClose,
  onImportSampleLegacy,
}) => {
  const [selectedFormat, setSelectedFormat] = useState<LegacyFormatInfo>(LEGACY_FORMATS[0]);
  const [hexInput, setHexInput] = useState('CF AD 12 FE');
  const [sniffResult, setSniffResult] = useState<LegacyFormatInfo | null>(LEGACY_FORMATS[0]);
  const [searchFilter, setSearchFilter] = useState('');
  const [convertedNotice, setConvertedNotice] = useState<string | null>(null);

  if (!isOpen) return null;

  const filteredFormats = LEGACY_FORMATS.filter(
    f =>
      f.name.toLowerCase().includes(searchFilter.toLowerCase()) ||
      f.extension.toLowerCase().includes(searchFilter.toLowerCase()) ||
      f.typicalClient.toLowerCase().includes(searchFilter.toLowerCase())
  );

  const handleSniff = () => {
    const clean = hexInput.replace(/\s+/g, '').toUpperCase();
    const match = LEGACY_FORMATS.find(f => {
      const sigClean = f.signatureHex.replace(/\s+/g, '').toUpperCase();
      return clean.startsWith(sigClean) || sigClean.startsWith(clean);
    });
    setSniffResult(match || null);
  };

  const handleConvertSample = (fmt: LegacyFormatInfo) => {
    const newMsg: EmailMessage = {
      id: `legacy_${Date.now()}`,
      messageId: `<${Date.now()}@${fmt.id}.converted.local>`,
      folderId: 'inbox',
      subject: `[Imported from ${fmt.name}] Legacy Corporate Correspondence Archive`,
      from: { name: `${fmt.name} Extractor`, email: `import@${fmt.id}.local` },
      to: [{ name: 'Current User', email: 'adpaolucci@corp.local' }],
      date: new Date().toISOString(),
      sizeBytes: 1024 * 350,
      bodyText: `This email was extracted from legacy ${fmt.name} (${fmt.extension}) container via the mail2mail native Rust decoder without requiring the original host software (${fmt.typicalClient}).\n\nConversion Pipeline: ${fmt.conversionMethod}\nContainer Signature: ${fmt.signatureHex}`,
      attachments: [],
      mapiProperties: [
        { tag: '0x001A001F', name: 'PR_MESSAGE_CLASS_W', type: 'PT_UNICODE', value: 'IPM.Note' },
        { tag: '0x0037001F', name: 'PR_SUBJECT_W', type: 'PT_UNICODE', value: `[Imported from ${fmt.name}]` },
      ],
      isRead: false,
      isFlagged: true,
      importance: 'high',
      sha256: '9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b',
      md5: '8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e',
      sourceFormat: fmt.id === 'dbx' ? 'DBX' : fmt.id === 'nsf' ? 'NSF' : 'EML',
    };

    onImportSampleLegacy(newMsg);
    setConvertedNotice(`Converted 1 item from ${fmt.name} and injected into your active mailbox workspace!`);
    setTimeout(() => setConvertedNotice(null), 3000);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-4xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-indigo-600/30 border border-indigo-500/40 flex items-center justify-center text-indigo-400">
              <Layers className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Obsolete & Specialized Legacy Formats</h2>
              <p className="text-[11px] text-slate-400">
                14+ Legacy container decoders &bull; Magic byte identification &bull; Direct conversion to MS-PST
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

        {/* Body (Split Left: Format List, Right: Details & Sniffer) */}
        <div className="flex-1 flex overflow-hidden">
          {/* Left Column: Format Directory */}
          <div className="w-80 bg-slate-950 border-r border-slate-800 flex flex-col">
            <div className="p-2 border-b border-slate-800">
              <div className="relative">
                <Search className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-1/2 -translate-y-1/2" />
                <input
                  type="text"
                  placeholder="Filter legacy formats..."
                  value={searchFilter}
                  onChange={(e) => setSearchFilter(e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded pl-8 pr-2.5 py-1 text-slate-100 placeholder-slate-400 text-xs"
                />
              </div>
            </div>

            <div className="flex-1 overflow-y-auto divide-y divide-slate-850">
              {filteredFormats.map((fmt) => {
                const isSelected = selectedFormat.id === fmt.id;
                return (
                  <div
                    key={fmt.id}
                    onClick={() => {
                      setSelectedFormat(fmt);
                      setHexInput(fmt.signatureHex);
                      setSniffResult(fmt);
                    }}
                    className={`p-2.5 cursor-pointer transition-colors ${
                      isSelected
                        ? 'bg-indigo-600/25 border-l-2 border-indigo-500 text-white font-medium'
                        : 'hover:bg-slate-900/60 text-slate-300'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-semibold text-xs">{fmt.name}</span>
                      <span className="font-mono text-[10px] text-indigo-400 px-1 rounded bg-indigo-950/60 border border-indigo-800/40">
                        {fmt.extension}
                      </span>
                    </div>
                    <div className="text-[10px] text-slate-500 mt-0.5 truncate">{fmt.typicalClient}</div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Right Column: Format Specs & Sniffer Workbench */}
          <div className="flex-1 p-5 overflow-y-auto space-y-4">
            {convertedNotice && (
              <div className="bg-emerald-950/70 border border-emerald-600/50 rounded-lg p-3 text-emerald-300 text-[11px] flex items-center space-x-2">
                <CheckCircle className="w-4 h-4 text-emerald-400 shrink-0" />
                <span>{convertedNotice}</span>
              </div>
            )}

            {/* Selected Format Details Card */}
            <div className="bg-slate-800/60 border border-slate-700 rounded-lg p-4 space-y-3">
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="text-sm font-semibold text-white tracking-tight">{selectedFormat.name}</h3>
                  <p className="text-[11px] text-slate-400">{selectedFormat.typicalClient} &bull; {selectedFormat.era}</p>
                </div>
                <button
                  onClick={() => handleConvertSample(selectedFormat)}
                  className="px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5"
                >
                  <Cpu className="w-3.5 h-3.5" />
                  <span>Convert Sample to Active PST</span>
                </button>
              </div>

              <p className="text-slate-300 text-xs leading-relaxed">{selectedFormat.description}</p>

              <div className="grid grid-cols-2 gap-3 pt-2 font-mono text-[11px]">
                <div className="bg-slate-900 p-2.5 rounded border border-slate-800">
                  <span className="text-slate-500 text-[10px] block font-sans">Magic Header Signature</span>
                  <span className="text-indigo-400 font-bold">{selectedFormat.signatureHex}</span>
                </div>
                <div className="bg-slate-900 p-2.5 rounded border border-slate-800">
                  <span className="text-slate-500 text-[10px] block font-sans">Direct Conversion Target</span>
                  <span className="text-emerald-400 font-bold">MS-PST Unicode (Outlook 2010+)</span>
                </div>
              </div>

              <div className="bg-slate-950 p-3 rounded border border-slate-800 text-[11px]">
                <span className="text-slate-400 font-semibold block mb-1">Conversion Architecture:</span>
                <span className="text-slate-300">{selectedFormat.conversionMethod}</span>
              </div>
            </div>

            {/* Interactive Magic Byte Sniffer */}
            <div className="bg-slate-950 border border-slate-800 rounded-lg p-4 space-y-3">
              <div className="flex items-center space-x-2 text-indigo-400 font-semibold">
                <Binary className="w-4 h-4" />
                <span>Magic Byte Sniffer & Header Inspector</span>
              </div>

              <div className="flex items-center space-x-2">
                <input
                  type="text"
                  value={hexInput}
                  onChange={(e) => setHexInput(e.target.value)}
                  placeholder="Enter hex bytes (e.g. CF AD 12 FE or 1A 00 00 04)"
                  className="flex-1 bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                />
                <button
                  onClick={handleSniff}
                  className="px-3 py-2 rounded bg-indigo-600 hover:bg-indigo-500 text-white font-medium flex items-center space-x-1"
                >
                  <Search className="w-3.5 h-3.5" />
                  <span>Sniff Signature</span>
                </button>
              </div>

              {sniffResult ? (
                <div className="p-3 bg-slate-900 border border-emerald-500/40 rounded-lg flex items-center justify-between text-xs font-mono">
                  <div className="flex items-center space-x-2">
                    <CheckCircle className="w-4 h-4 text-emerald-400 shrink-0" />
                    <span>Identified Format: <strong className="text-white">{sniffResult.name}</strong></span>
                  </div>
                  <span className="text-slate-400">{sniffResult.typicalClient}</span>
                </div>
              ) : (
                <div className="p-3 bg-slate-900 border border-slate-800 rounded-lg text-slate-500 italic text-[11px]">
                  No matching legacy format for this signature. Try: <code>CF AD 12 FE</code> (DBX) or <code>1A 00 00 04</code> (NSF).
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <span className="text-slate-400 text-[11px]">
            Zero-Outlook independent parsers powered by mail2mail Rust engine
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
