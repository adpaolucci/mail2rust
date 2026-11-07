import React, { useState } from 'react';
import {
  FileCode,
  X,
  CheckCircle2,
  Download,
  HardDrive,
  Cpu,
  Layers,
  ShieldCheck,
  Binary,
} from 'lucide-react';
import { MailboxFolder, EmailMessage } from '../types/mail';
import { buildDirectPst, PstBuildResult } from '../utils/pstGenerator';

interface DirectPstModalProps {
  isOpen: boolean;
  onClose: () => void;
  folders: MailboxFolder[];
  messages: EmailMessage[];
}

export const DirectPstModal: React.FC<DirectPstModalProps> = ({
  isOpen,
  onClose,
  folders,
  messages,
}) => {
  const [targetVersion, setTargetVersion] = useState<'UNICODE_2010' | 'UNICODE_2003' | 'ANSI_LEGACY'>('UNICODE_2010');
  const [compression, setCompression] = useState<'compressable' | 'none'>('compressable');
  const [includeDeleted, setIncludeDeleted] = useState(false);
  const [isBuilding, setIsBuilding] = useState(false);
  const [buildResult, setBuildResult] = useState<PstBuildResult | null>(null);

  if (!isOpen) return null;

  const selectedFolders = folders.filter(f => f.selected);
  const selectedFolderIds = new Set(selectedFolders.map(f => f.id));
  const messagesToInclude = messages.filter(m => selectedFolderIds.has(m.folderId));

  const handleGeneratePst = () => {
    setIsBuilding(true);
    setTimeout(() => {
      const res = buildDirectPst(selectedFolders, messagesToInclude, {
        targetVersion,
        compression,
        creatorApp: 'mail2mail-rust-direct-mapi v3.2.0',
      });
      setBuildResult(res);
      setIsBuilding(false);
    }, 600);
  };

  const handleDownloadPst = () => {
    if (!buildResult) return;
    const blob = new Blob([buildResult.binaryBuffer.buffer as ArrayBuffer], { type: 'application/vnd.ms-outlook' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `mail2mail_Direct_${Date.now()}.pst`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-2xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-blue-600/30 border border-blue-500/40 flex items-center justify-center text-blue-400">
              <FileCode className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Direct PST Generation Studio</h2>
              <p className="text-[11px] text-slate-400">
                Independent MAPI / libpff pure builder &bull; No Microsoft Outlook or Windows DLLs required
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

        {/* Body */}
        <div className="p-5 overflow-y-auto space-y-5">
          {/* Architecture Banner */}
          <div className="bg-blue-950/40 border border-blue-600/30 rounded-lg p-3 text-[11px] text-blue-200 flex items-start space-x-3">
            <Cpu className="w-4 h-4 text-blue-400 shrink-0 mt-0.5" />
            <div>
              <p className="font-semibold text-blue-100">Pure-Rust [MS-PST] Specification Engine</p>
              <p className="text-blue-300/90 mt-0.5 leading-relaxed">
                Constructs complete 64-bit NDB (Node Database) layer, B-Trees (NBT/BBT), and LTP (Local Tables and Properties) Property Context (PC) and Table Context (TC) structures directly in memory. Fully platform independent.
              </p>
            </div>
          </div>

          {/* Configuration Options */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label className="block font-medium text-slate-300 mb-1.5">Target PST Format</label>
              <select
                value={targetVersion}
                onChange={(e) => setTargetVersion(e.target.value as any)}
                className="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-slate-100 focus:outline-none focus:border-blue-500"
              >
                <option value="UNICODE_2010">Unicode PST (Outlook 2010 - 2024 / M365) [Max 50 GB]</option>
                <option value="UNICODE_2003">Unicode PST (Outlook 2003 - 2007) [Max 20 GB]</option>
                <option value="ANSI_LEGACY">Legacy ANSI PST (Outlook 97 - 2002) [Max 2 GB limit]</option>
              </select>
            </div>

            <div>
              <label className="block font-medium text-slate-300 mb-1.5">Encryption / Compression</label>
              <select
                value={compression}
                onChange={(e) => setCompression(e.target.value as any)}
                className="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-slate-100 focus:outline-none focus:border-blue-500"
              >
                <option value="compressable">Compressible Encryption (NDB_CRYPT_PERMUTE)</option>
                <option value="none">No Encryption (NDB_CRYPT_NONE)</option>
              </select>
            </div>
          </div>

          {/* Inclusion Summary */}
          <div className="bg-slate-800/60 border border-slate-700/80 rounded-lg p-3">
            <span className="font-medium text-slate-200">Selected Folders & Items</span>
            <div className="mt-2 grid grid-cols-3 gap-2 text-center text-slate-300">
              <div className="bg-slate-900 p-2 rounded border border-slate-800">
                <div className="text-[10px] text-slate-400">Folders</div>
                <div className="text-sm font-bold text-white font-mono">{selectedFolders.length}</div>
              </div>
              <div className="bg-slate-900 p-2 rounded border border-slate-800">
                <div className="text-[10px] text-slate-400">Total Items</div>
                <div className="text-sm font-bold text-blue-400 font-mono">{messagesToInclude.length}</div>
              </div>
              <div className="bg-slate-900 p-2 rounded border border-slate-800">
                <div className="text-[10px] text-slate-400">Estimated Size</div>
                <div className="text-sm font-bold text-emerald-400 font-mono">
                  {(messagesToInclude.reduce((s, m) => s + m.sizeBytes, 0) / (1024 * 1024)).toFixed(2)} MB
                </div>
              </div>
            </div>
          </div>

          {/* Generated Result Preview */}
          {buildResult && (
            <div className="bg-slate-950 border border-emerald-500/40 rounded-lg p-4 space-y-3 font-mono">
              <div className="flex items-center space-x-2 text-emerald-400 font-semibold text-xs">
                <CheckCircle2 className="w-4 h-4" />
                <span>PST Binary Constructed Successfully</span>
              </div>

              <div className="grid grid-cols-2 gap-2 text-[11px] text-slate-300">
                <div>Magic Header: <span className="text-blue-400">{buildResult.pstHeader.magic}</span></div>
                <div>Format Version: <span className="text-amber-400">{buildResult.pstHeader.version}</span></div>
                <div>CRC-32 Checksum: <span className="text-emerald-400">{buildResult.pstHeader.crc32}</span></div>
                <div>Encryption: <span className="text-purple-400">{buildResult.pstHeader.encryption}</span></div>
                <div>File Size: <span className="text-slate-100">{buildResult.totalBytes.toLocaleString()} bytes</span></div>
                <div>Root Store NID: <span className="text-slate-100">0x21 (NID_MESSAGE_STORE)</span></div>
              </div>

              <button
                onClick={handleDownloadPst}
                className="w-full py-2 px-4 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium flex items-center justify-center space-x-2 transition-colors shadow-sm"
              >
                <Download className="w-4 h-4" />
                <span>Download Generated .PST Archive</span>
              </button>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2 text-slate-400 text-[11px]">
            <ShieldCheck className="w-4 h-4 text-emerald-400" />
            <span>Zero Outlook requirement verified</span>
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg border border-slate-700 hover:bg-slate-800 text-slate-300 transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleGeneratePst}
              disabled={isBuilding}
              className="px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5 disabled:opacity-50"
            >
              <Cpu className="w-3.5 h-3.5" />
              <span>{isBuilding ? 'Generating PST...' : 'Build MS-PST File'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
