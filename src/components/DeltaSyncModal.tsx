import React, { useState } from 'react';
import {
  GitCommit,
  X,
  CheckCircle,
  RefreshCw,
  Clock,
  HardDrive,
  Database,
  ArrowRight,
  Shield,
  Layers,
} from 'lucide-react';
import { SyncCheckpoint, EmailMessage } from '../types/mail';
import { calculateDeltaDiff, createUpdatedCheckpoint } from '../utils/deltaSync';

interface DeltaSyncModalProps {
  isOpen: boolean;
  onClose: () => void;
  checkpoint: SyncCheckpoint | null;
  onUpdateCheckpoint: (chk: SyncCheckpoint) => void;
  currentMessages: EmailMessage[];
}

export const DeltaSyncModal: React.FC<DeltaSyncModalProps> = ({
  isOpen,
  onClose,
  checkpoint,
  onUpdateCheckpoint,
  currentMessages,
}) => {
  const [isSyncing, setIsSyncing] = useState(false);
  const [syncSuccessMsg, setSyncSuccessMsg] = useState<string | null>(null);

  if (!isOpen) return null;

  const diff = calculateDeltaDiff(currentMessages, checkpoint);

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const handleRunDeltaSync = () => {
    setIsSyncing(true);
    setSyncSuccessMsg(null);

    setTimeout(() => {
      const updated = createUpdatedCheckpoint(
        checkpoint,
        diff.newMessages,
        'Project_Titan_Source',
        'Target_Unicode_PST'
      );
      onUpdateCheckpoint(updated);
      setIsSyncing(false);
      setSyncSuccessMsg(`Delta sync completed! Committed ${diff.newCount} new items (${formatSize(diff.newSizeBytes)}).`);
    }, 700);
  };

  const handleResetCheckpoints = () => {
    onUpdateCheckpoint({
      id: `chk_${Date.now()}`,
      timestamp: new Date().toISOString(),
      sourceId: 'Reset',
      targetId: 'Target',
      migratedMessageIds: [],
      folderHighUids: {},
      totalBytesMigrated: 0,
      totalMessagesMigrated: 0,
      status: 'completed',
    });
    setSyncSuccessMsg('Checkpoint state cleared. Next sync will be a full initial migration.');
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-2xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-cyan-600/30 border border-cyan-500/40 flex items-center justify-center text-cyan-400">
              <GitCommit className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Incremental / Delta Migration Checkpoints</h2>
              <p className="text-[11px] text-slate-400">
                Track folder UID watermarks and Message-ID hashes to migrate only new or modified emails
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
          {/* Overview Metrics Card */}
          <div className="grid grid-cols-3 gap-3 text-center">
            <div className="bg-slate-800/60 border border-slate-700 rounded-lg p-3">
              <span className="text-[10px] text-slate-400">Previously Migrated</span>
              <p className="text-base font-bold text-slate-100 font-mono mt-0.5">
                {diff.alreadyMigratedCount}
              </p>
              <span className="text-[10px] text-slate-500">Committed to PST</span>
            </div>

            <div className="bg-slate-800/60 border border-slate-700 rounded-lg p-3">
              <span className="text-[10px] text-slate-400">Pending Delta Items</span>
              <p className="text-base font-bold text-cyan-400 font-mono mt-0.5">
                {diff.newCount}
              </p>
              <span className="text-[10px] text-cyan-500">{formatSize(diff.newSizeBytes)}</span>
            </div>

            <div className="bg-slate-800/60 border border-slate-700 rounded-lg p-3">
              <span className="text-[10px] text-slate-400">Total Live Mailbox</span>
              <p className="text-base font-bold text-slate-100 font-mono mt-0.5">
                {currentMessages.length}
              </p>
              <span className="text-[10px] text-slate-500">Across active folders</span>
            </div>
          </div>

          {/* Active Checkpoint Watermark */}
          <div className="bg-slate-950 border border-slate-800 rounded-lg p-3.5 space-y-2 font-mono text-[11px]">
            <div className="flex items-center justify-between text-slate-300 font-bold border-b border-slate-800 pb-2">
              <span className="flex items-center space-x-1.5">
                <Database className="w-3.5 h-3.5 text-cyan-400" />
                <span>Active Watermark Checkpoint</span>
              </span>
              <span className="text-slate-400">{checkpoint ? checkpoint.id : 'No Checkpoint Set'}</span>
            </div>

            <div className="grid grid-cols-2 gap-2 text-slate-400">
              <div>Last Sync Run: <span className="text-slate-200">{checkpoint ? new Date(checkpoint.timestamp).toUTCString() : 'Never'}</span></div>
              <div>Total Migrated: <span className="text-emerald-400">{checkpoint?.totalMessagesMigrated || 0} items</span></div>
              <div>Source Container: <span className="text-slate-200">{checkpoint?.sourceId || 'None'}</span></div>
              <div>Cumulative Size: <span className="text-blue-400">{formatSize(checkpoint?.totalBytesMigrated || 0)}</span></div>
            </div>
          </div>

          {/* Pending Delta Items Preview */}
          <div className="space-y-2">
            <span className="font-semibold text-slate-200 flex items-center space-x-1.5">
              <Layers className="w-4 h-4 text-cyan-400" />
              <span>Pending New Items to Migrate in This Delta Run</span>
            </span>

            {diff.newMessages.length === 0 ? (
              <div className="bg-slate-800/40 border border-slate-700/60 rounded-lg p-4 text-center text-slate-400">
                <CheckCircle className="w-6 h-6 text-emerald-400 mx-auto mb-1" />
                <p className="font-medium text-slate-200">Everything is fully up to date</p>
                <p className="text-[11px] text-slate-500">No new emails have arrived since the last sync checkpoint.</p>
              </div>
            ) : (
              <div className="bg-slate-950 border border-slate-800 rounded-lg divide-y divide-slate-850 max-h-44 overflow-y-auto">
                {diff.newMessages.map((msg) => (
                  <div key={msg.id} className="p-2.5 flex items-center justify-between hover:bg-slate-900/60">
                    <div className="min-w-0 pr-2">
                      <div className="text-slate-200 font-medium truncate">{msg.subject}</div>
                      <div className="text-[10px] text-slate-500 truncate">From: {msg.from.email}</div>
                    </div>
                    <div className="shrink-0 text-right font-mono text-[10px]">
                      <span className="text-cyan-400 block">{formatSize(msg.sizeBytes)}</span>
                      <span className="text-slate-500">{msg.sourceFormat}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {syncSuccessMsg && (
            <div className="bg-emerald-950/60 border border-emerald-600/50 rounded-lg p-3 text-emerald-300 text-[11px] flex items-center space-x-2">
              <CheckCircle className="w-4 h-4 text-emerald-400 shrink-0" />
              <span>{syncSuccessMsg}</span>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <button
            onClick={handleResetCheckpoints}
            className="text-slate-400 hover:text-rose-400 text-[11px] hover:underline"
          >
            Clear Checkpoint History
          </button>

          <div className="flex items-center space-x-2">
            <button
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg border border-slate-700 hover:bg-slate-800 text-slate-300 transition-colors"
            >
              Close
            </button>
            <button
              onClick={handleRunDeltaSync}
              disabled={isSyncing || diff.newCount === 0}
              className="px-4 py-1.5 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5 disabled:opacity-50"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isSyncing ? 'animate-spin' : ''}`} />
              <span>{isSyncing ? 'Synchronizing...' : `Run Delta Sync (${diff.newCount} Items)`}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
