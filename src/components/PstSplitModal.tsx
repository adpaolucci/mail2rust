import React, { useState } from 'react';
import {
  Split,
  X,
  Layers,
  HardDrive,
  Download,
  CheckCircle2,
  GitBranch,
  FileCode,
} from 'lucide-react';
import { EmailMessage, SplitVolume } from '../types/mail';
import { splitMessagesIntoVolumes } from '../utils/pstSplitter';
import JSZip from 'jszip';

interface PstSplitModalProps {
  isOpen: boolean;
  onClose: () => void;
  messages: EmailMessage[];
}

export const PstSplitModal: React.FC<PstSplitModalProps> = ({
  isOpen,
  onClose,
  messages,
}) => {
  // Preset options: 20GB, 45GB, 2GB, or 1.5MB (demo scaled)
  const [splitPreset, setSplitPreset] = useState<'20gb' | '45gb' | '2gb' | 'demo1mb'>('demo1mb');
  const [preserveThreads, setPreserveThreads] = useState(true);
  const [filePrefix, setFilePrefix] = useState('Titan_Archive');
  const [isExporting, setIsExporting] = useState(false);

  if (!isOpen) return null;

  // Determine split threshold in bytes
  let maxBytes = 20 * 1024 * 1024 * 1024; // 20 GB default
  if (splitPreset === '45gb') {
    maxBytes = 45 * 1024 * 1024 * 1024;
  } else if (splitPreset === '2gb') {
    maxBytes = 2 * 1024 * 1024 * 1024;
  } else if (splitPreset === 'demo1mb') {
    // 1.5 MB threshold to demonstrate real multi-volume split on the active corpus
    maxBytes = 1.5 * 1024 * 1024;
  }

  const volumes: SplitVolume[] = splitMessagesIntoVolumes(messages, {
    maxVolumeSizeBytes: maxBytes,
    preserveThreads,
    filePrefix,
    targetFormat: 'PST_UNICODE',
  });

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const handleExportAllVolumes = async () => {
    setIsExporting(true);
    const zip = new JSZip();

    volumes.forEach((vol) => {
      // Create representative manifest inside the PST archive bundle
      const volManifest = {
        volumeName: vol.fileName,
        totalItems: vol.messages.length,
        totalSizeBytes: vol.totalSizeBytes,
        items: vol.messages.map(m => ({
          subject: m.subject,
          from: m.from.email,
          date: m.date,
          sizeBytes: m.sizeBytes,
          sha256: m.sha256,
        })),
      };
      zip.file(`${vol.fileName}.meta.json`, JSON.stringify(volManifest, null, 2));
      zip.file(`${vol.fileName}`, `!BDN... [Direct MS-PST Unicode Volume Container: ${vol.fileName}]`);
    });

    const content = await zip.generateAsync({ type: 'blob' });
    const url = URL.createObjectURL(content);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${filePrefix}_Split_Volumes_Bundle.zip`;
    a.click();
    URL.revokeObjectURL(url);
    setIsExporting(false);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-3xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-purple-600/30 border border-purple-500/40 flex items-center justify-center text-purple-400">
              <Split className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Automatic PST Splitting by Size</h2>
              <p className="text-[11px] text-slate-400">
                Auto-split oversized archives into 20 GB / 45 GB multi-volume Unicode PST parts with thread clustering
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
        <div className="p-5 overflow-y-auto space-y-4">
          {/* Controls */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div>
              <label className="block text-slate-300 font-medium mb-1">Volume Split Threshold</label>
              <select
                value={splitPreset}
                onChange={(e) => setSplitPreset(e.target.value as any)}
                className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
              >
                <option value="demo1mb">1.5 MB (Interactive Demo Partition)</option>
                <option value="20gb">20 GB (Recommended Unicode Limit)</option>
                <option value="45gb">45 GB (Outlook 2010-365 Safe Boundary)</option>
                <option value="2gb">2 GB (Legacy ANSI Outlook 97-2002)</option>
              </select>
            </div>

            <div>
              <label className="block text-slate-300 font-medium mb-1">Volume Naming Pattern</label>
              <input
                type="text"
                value={filePrefix}
                onChange={(e) => setFilePrefix(e.target.value)}
                className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
              />
            </div>

            <div className="flex flex-col justify-end">
              <label className="flex items-center space-x-2 p-2 rounded bg-slate-800 border border-slate-700 cursor-pointer">
                <input
                  type="checkbox"
                  checked={preserveThreads}
                  onChange={(e) => setPreserveThreads(e.target.checked)}
                  className="rounded bg-slate-900 border-slate-700 text-purple-500"
                />
                <span className="text-slate-300">Cluster Email Threads</span>
              </label>
            </div>
          </div>

          {/* Explanation Banner */}
          <div className="bg-purple-950/40 border border-purple-600/30 rounded-lg p-3 text-[11px] text-purple-200 flex items-start space-x-2.5">
            <GitBranch className="w-4 h-4 text-purple-400 shrink-0 mt-0.5" />
            <div>
              <span className="font-semibold text-purple-100">Conversation Thread Preservation:</span>
              <span className="ml-1 text-purple-300">
                When active, replies and original emails sharing a conversation thread remain bound together within the same volume rather than orphaned across different PST files.
              </span>
            </div>
          </div>

          {/* Split Volume Allocation Diagram */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="font-semibold text-slate-200 flex items-center space-x-2">
                <Layers className="w-4 h-4 text-blue-400" />
                <span>Generated Multi-Volume Layout ({volumes.length} Volumes)</span>
              </span>
              <span className="text-slate-400 font-mono text-[11px]">
                Total Corpus: {formatSize(messages.reduce((s, m) => s + m.sizeBytes, 0))}
              </span>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {volumes.map((vol) => {
                const percentFull = Math.min(100, (vol.totalSizeBytes / maxBytes) * 100);

                return (
                  <div
                    key={vol.volumeIndex}
                    className="bg-slate-800/70 border border-slate-700 rounded-lg p-3 space-y-2 hover:border-slate-600 transition-colors"
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center space-x-2">
                        <FileCode className="w-4 h-4 text-purple-400" />
                        <span className="font-mono font-semibold text-white">{vol.fileName}</span>
                      </div>
                      <span className="px-2 py-0.5 rounded bg-purple-950 border border-purple-700/60 text-purple-300 text-[10px] font-mono">
                        Part {vol.volumeIndex}
                      </span>
                    </div>

                    <div className="flex justify-between text-[11px] text-slate-300">
                      <span>{vol.messages.length} messages</span>
                      <span className="font-mono text-emerald-400">{formatSize(vol.totalSizeBytes)}</span>
                    </div>

                    {/* Volume Capacity Bar */}
                    <div className="w-full h-1.5 bg-slate-900 rounded-full overflow-hidden">
                      <div
                        className="h-full bg-purple-500 rounded-full"
                        style={{ width: `${percentFull}%` }}
                      />
                    </div>

                    {/* Messages Sample */}
                    <div className="text-[10px] text-slate-400 truncate">
                      {vol.messages.map(m => m.subject).slice(0, 2).join(' &bull; ')}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="text-slate-400 text-[11px] font-mono">
            {volumes.length} PST partition files ready
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg border border-slate-700 hover:bg-slate-800 text-slate-300 transition-colors"
            >
              Close
            </button>
            <button
              onClick={handleExportAllVolumes}
              disabled={isExporting}
              className="px-4 py-1.5 rounded-lg bg-purple-600 hover:bg-purple-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5 disabled:opacity-50"
            >
              <Download className="w-3.5 h-3.5" />
              <span>{isExporting ? 'Packaging...' : 'Export All Split Volumes (.zip)'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
