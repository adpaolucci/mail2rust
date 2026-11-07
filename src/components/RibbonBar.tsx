import React from 'react';
import {
  FileCode,
  CloudLightning,
  Filter,
  Split,
  GitCommit,
  ShieldAlert,
  Layers,
  Terminal,
  Play,
  RotateCcw,
} from 'lucide-react';
import { FilterCriteria } from '../types/mail';

interface RibbonBarProps {
  onOpenDirectPst: () => void;
  onOpenCloudMigration: () => void;
  onOpenFilter: () => void;
  onOpenSplitter: () => void;
  onOpenDeltaSync: () => void;
  onOpenForensics: () => void;
  onOpenLegacyFormats: () => void;
  onOpenRustCodebase: () => void;
  onOpenExportReport: () => void;
  onOpenSecurityAudit: () => void;
  filterCriteria: FilterCriteria;
  onResetFilters: () => void;
  totalFilteredCount: number;
  totalOriginalCount: number;
}

export const RibbonBar: React.FC<RibbonBarProps> = ({
  onOpenDirectPst,
  onOpenCloudMigration,
  onOpenFilter,
  onOpenSplitter,
  onOpenDeltaSync,
  onOpenForensics,
  onOpenLegacyFormats,
  onOpenRustCodebase,
  onOpenExportReport,
  onOpenSecurityAudit,
  filterCriteria,
  onResetFilters,
  totalFilteredCount,
  totalOriginalCount,
}) => {
  const hasActiveFilters = Boolean(
    filterCriteria.startDate ||
    filterCriteria.endDate ||
    filterCriteria.senderSearch ||
    filterCriteria.recipientSearch ||
    filterCriteria.subjectSearch ||
    filterCriteria.hasAttachments !== null ||
    filterCriteria.minSizeBytes !== null ||
    filterCriteria.maxSizeBytes !== null ||
    (filterCriteria.importance && filterCriteria.importance !== 'all') ||
    (filterCriteria.readStatus && filterCriteria.readStatus !== 'all') ||
    filterCriteria.flaggedOnly ||
    filterCriteria.legalHoldOnly
  );

  return (
    <div className="bg-slate-800/90 border-b border-slate-700/80 px-3 py-2 flex flex-wrap items-center justify-between gap-2 text-xs">
      {/* Primary Action Buttons */}
      <div className="flex items-center flex-wrap gap-1.5">
        {/* 1. Direct PST Generation */}
        <button
          onClick={onOpenDirectPst}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white font-medium shadow-sm transition-all active:scale-[0.98]"
          title="Direct PST Generation Without Outlook Installed (Independent MAPI / libpff)"
        >
          <FileCode className="w-4 h-4 text-blue-200" />
          <span>Direct PST Builder</span>
        </button>

        {/* 2. Direct Cloud / SaaS Migrations */}
        <button
          onClick={onOpenCloudMigration}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-650 text-slate-100 border border-slate-600/70 font-medium transition-all"
          title="Direct Cloud / SaaS Migrations (Exchange, Microsoft 365, Google Workspace, IMAP)"
        >
          <CloudLightning className="w-4 h-4 text-emerald-400" />
          <span>Cloud Migration</span>
        </button>

        {/* 3. Filtering */}
        <button
          onClick={onOpenFilter}
          className={`flex items-center space-x-1.5 px-3 py-1.5 rounded border font-medium transition-all ${
            hasActiveFilters
              ? 'bg-amber-950/80 border-amber-500/70 text-amber-200'
              : 'bg-slate-700 hover:bg-slate-650 text-slate-100 border-slate-600/70'
          }`}
          title="Date, Sender, and Metadata Filtering"
        >
          <Filter className={`w-4 h-4 ${hasActiveFilters ? 'text-amber-400' : 'text-blue-400'}`} />
          <span>Filter Rules</span>
          {hasActiveFilters && (
            <span className="ml-1 px-1.5 py-0.2 bg-amber-500/30 text-amber-300 rounded text-[10px] font-mono">
              Active
            </span>
          )}
        </button>

        {/* 4. Auto-Split PST */}
        <button
          onClick={onOpenSplitter}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-650 text-slate-100 border border-slate-600/70 font-medium transition-all"
          title="Automatic PST Splitting by Size (e.g., 20 GB / 45 GB Auto-Splitting)"
        >
          <Split className="w-4 h-4 text-purple-400" />
          <span>Auto-Split PST</span>
        </button>

        {/* 5. Incremental / Delta Sync */}
        <button
          onClick={onOpenDeltaSync}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-650 text-slate-100 border border-slate-600/70 font-medium transition-all"
          title="Incremental / Delta Migration (Sync Checkpoints)"
        >
          <GitCommit className="w-4 h-4 text-cyan-400" />
          <span>Delta Sync</span>
        </button>

        {/* 7. Forensics & eDiscovery */}
        <button
          onClick={onOpenForensics}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-650 text-slate-100 border border-slate-600/70 font-medium transition-all"
          title="Forensic, Legal, and eDiscovery Features (SHA-256, Bates Numbering, Chain of Custody)"
        >
          <ShieldAlert className="w-4 h-4 text-amber-400" />
          <span>Forensics & Legal</span>
        </button>

        {/* 8. Legacy Formats */}
        <button
          onClick={onOpenLegacyFormats}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-650 text-slate-100 border border-slate-600/70 font-medium transition-all"
          title="Obsolete / Specialized Legacy Formats (DBX, NSF, EMLX, TBB, etc.)"
        >
          <Layers className="w-4 h-4 text-indigo-400" />
          <span>Legacy Formats</span>
        </button>

        {/* 9. eDiscovery Compliance Report (PDF / CSV) */}
        <button
          onClick={onOpenExportReport}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-650 text-slate-100 border border-slate-600/70 font-medium transition-all"
          title="Export Professional eDiscovery Compliance Report (PDF / CSV Summary & Error Logs)"
        >
          <FileCode className="w-4 h-4 text-emerald-400" />
          <span>Export Report (PDF/CSV)</span>
        </button>

        {/* 10. Dedicated Security Audit Viewer */}
        <button
          onClick={onOpenSecurityAudit}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-650 text-slate-100 border border-slate-600/70 font-medium transition-all"
          title="Real-Time Security & Forensic Activity Audit Trail (ISO 27001 & FRCP 37(e))"
        >
          <ShieldAlert className="w-4 h-4 text-purple-400" />
          <span>Security Audit</span>
        </button>
      </div>

      {/* Right: Rust Workspace & Filter Status */}
      <div className="flex items-center space-x-2">
        {hasActiveFilters && (
          <div className="flex items-center space-x-1.5 bg-amber-950/50 border border-amber-600/40 rounded px-2.5 py-1 text-amber-300 text-[11px]">
            <span>Showing {totalFilteredCount} of {totalOriginalCount}</span>
            <button
              onClick={onResetFilters}
              className="p-0.5 hover:bg-amber-800/60 rounded text-amber-200"
              title="Reset all filters"
            >
              <RotateCcw className="w-3 h-3" />
            </button>
          </div>
        )}

        <button
          onClick={onOpenRustCodebase}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-orange-600 hover:bg-orange-500 text-white font-medium shadow-sm transition-all"
          title="View and download full Rust workspace codebase"
        >
          <Terminal className="w-4 h-4 text-orange-200" />
          <span>Rust Crate Code</span>
        </button>
      </div>
    </div>
  );
};
