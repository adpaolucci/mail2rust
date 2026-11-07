import React from 'react';
import { ShieldCheck, HardDrive, Cpu, Filter, CheckCircle2 } from 'lucide-react';
import { MailboxFolder, EmailMessage, FilterCriteria } from '../types/mail';

interface StatusBarProps {
  folders: MailboxFolder[];
  messages: EmailMessage[];
  activeFolder: MailboxFolder | null;
  hasActiveFilters: boolean;
  onOpenRustCodebase: () => void;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  folders,
  messages,
  activeFolder,
  hasActiveFilters,
  onOpenRustCodebase,
}) => {
  const selectedFolders = folders.filter(f => f.selected);
  const totalSizeBytes = messages.reduce((acc, m) => acc + m.sizeBytes, 0);

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  return (
    <div className="h-6 bg-slate-900 border-t border-slate-800 px-3 flex items-center justify-between text-[11px] text-slate-400 select-none font-mono">
      {/* Left: Selection Metrics */}
      <div className="flex items-center space-x-3">
        <div className="flex items-center space-x-1.5">
          <HardDrive className="w-3 h-3 text-blue-400" />
          <span className="text-slate-300">
            {activeFolder ? activeFolder.name : 'All Mailboxes'}: {messages.length} items
          </span>
        </div>

        <span className="text-slate-700">|</span>

        <span>
          Selected Folders: <span className="text-slate-200">{selectedFolders.length}/{folders.length}</span>
        </span>

        <span className="text-slate-700">|</span>

        <span>
          Corpus Size: <span className="text-emerald-400">{formatSize(totalSizeBytes)}</span>
        </span>

        {hasActiveFilters && (
          <>
            <span className="text-slate-700">|</span>
            <span className="text-amber-400 flex items-center space-x-1">
              <Filter className="w-3 h-3" />
              <span>Filters Active</span>
            </span>
          </>
        )}
      </div>

      {/* Right: Engine Architecture Info */}
      <div className="flex items-center space-x-3">
        <button
          onClick={onOpenRustCodebase}
          className="flex items-center space-x-1 text-orange-400 hover:text-orange-300"
          title="Click to view Rust crate code"
        >
          <Cpu className="w-3 h-3" />
          <span>mail2mail-rust-direct-mapi</span>
        </button>

        <span className="text-slate-700">|</span>

        <div className="flex items-center space-x-1 text-emerald-400">
          <ShieldCheck className="w-3 h-3" />
          <span>Zero-Outlook Independent</span>
        </div>

        <span className="text-slate-700">|</span>

        <span className="text-slate-400">Unicode MS-PST [64-bit NDB]</span>
      </div>
    </div>
  );
};
