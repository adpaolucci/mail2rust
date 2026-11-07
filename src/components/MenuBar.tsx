import React, { useState, useRef, useEffect } from 'react';
import {
  FileText,
  FolderOpen,
  Save,
  Split,
  RefreshCw,
  Search,
  Shield,
  Layers,
  Code,
  HelpCircle,
  ExternalLink,
  ChevronDown,
} from 'lucide-react';

interface MenuBarProps {
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
  onSelectAllFolders: () => void;
  onClearFolderSelection: () => void;
}

export const MenuBar: React.FC<MenuBarProps> = ({
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
  onSelectAllFolders,
  onClearFolderSelection,
}) => {
  const [activeMenu, setActiveMenu] = useState<string | null>(null);
  const menuBarRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuBarRef.current && !menuBarRef.current.contains(e.target as Node)) {
        setActiveMenu(null);
      }
    };
    window.addEventListener('mousedown', handleClickOutside);
    return () => window.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const toggleMenu = (menuName: string) => {
    setActiveMenu(activeMenu === menuName ? null : menuName);
  };

  return (
    <div
      ref={menuBarRef}
      className="bg-slate-850 bg-slate-900/95 border-b border-slate-700/60 px-2 py-0.5 flex items-center space-x-1 text-xs text-slate-300 select-none relative z-40"
    >
      {/* File Menu */}
      <div className="relative">
        <button
          onClick={() => toggleMenu('file')}
          className={`px-2.5 py-1 rounded transition-colors ${
            activeMenu === 'file' ? 'bg-slate-700 text-white' : 'hover:bg-slate-800 text-slate-200'
          }`}
        >
          File
        </button>
        {activeMenu === 'file' && (
          <div className="absolute left-0 top-full mt-1 w-64 bg-slate-800 border border-slate-700 rounded-md shadow-2xl py-1 z-50 text-slate-200">
            <button
              onClick={() => { setActiveMenu(null); onOpenDirectPst(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center justify-between text-xs"
            >
              <span className="flex items-center space-x-2">
                <Save className="w-3.5 h-3.5 text-blue-400" />
                <span>Direct PST Generation (No Outlook)</span>
              </span>
              <span className="text-[10px] text-slate-400 font-mono">Ctrl+P</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenCloudMigration(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center justify-between text-xs"
            >
              <span className="flex items-center space-x-2">
                <FolderOpen className="w-3.5 h-3.5 text-emerald-400" />
                <span>Cloud & SaaS Migration Wizard</span>
              </span>
              <span className="text-[10px] text-slate-400 font-mono">Ctrl+M</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenSplitter(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center justify-between text-xs"
            >
              <span className="flex items-center space-x-2">
                <Split className="w-3.5 h-3.5 text-purple-400" />
                <span>Auto-Split PST by Size (20GB / 45GB)</span>
              </span>
              <span className="text-[10px] text-slate-400 font-mono">Ctrl+S</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenExportReport(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center justify-between text-xs"
            >
              <span className="flex items-center space-x-2">
                <FileText className="w-3.5 h-3.5 text-emerald-400" />
                <span>Export eDiscovery Report (PDF / CSV)</span>
              </span>
              <span className="text-[10px] text-slate-400 font-mono">Ctrl+R</span>
            </button>
            <div className="h-px bg-slate-700 my-1" />
            <button
              onClick={() => { setActiveMenu(null); onOpenRustCodebase(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center space-x-2 text-xs"
            >
              <Code className="w-3.5 h-3.5 text-orange-400" />
              <span>Export Full Rust Workspace (.zip)</span>
            </button>
          </div>
        )}
      </div>

      {/* Edit Menu */}
      <div className="relative">
        <button
          onClick={() => toggleMenu('edit')}
          className={`px-2.5 py-1 rounded transition-colors ${
            activeMenu === 'edit' ? 'bg-slate-700 text-white' : 'hover:bg-slate-800 text-slate-200'
          }`}
        >
          Edit
        </button>
        {activeMenu === 'edit' && (
          <div className="absolute left-0 top-full mt-1 w-52 bg-slate-800 border border-slate-700 rounded-md shadow-2xl py-1 z-50 text-slate-200">
            <button
              onClick={() => { setActiveMenu(null); onSelectAllFolders(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center justify-between text-xs"
            >
              <span>Select All Folders</span>
              <span className="text-[10px] text-slate-400 font-mono">Ctrl+A</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onClearFolderSelection(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center justify-between text-xs"
            >
              <span>Clear Folder Selection</span>
              <span className="text-[10px] text-slate-400 font-mono">Esc</span>
            </button>
            <div className="h-px bg-slate-700 my-1" />
            <button
              onClick={() => { setActiveMenu(null); onOpenFilter(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white flex items-center space-x-2 text-xs"
            >
              <Search className="w-3.5 h-3.5 text-blue-400" />
              <span>Configure Filtering Rules...</span>
            </button>
          </div>
        )}
      </div>

      {/* Migration Menu */}
      <div className="relative">
        <button
          onClick={() => toggleMenu('migration')}
          className={`px-2.5 py-1 rounded transition-colors ${
            activeMenu === 'migration' ? 'bg-slate-700 text-white' : 'hover:bg-slate-800 text-slate-200'
          }`}
        >
          Migration
        </button>
        {activeMenu === 'migration' && (
          <div className="absolute left-0 top-full mt-1 w-64 bg-slate-800 border border-slate-700 rounded-md shadow-2xl py-1 z-50 text-slate-200">
            <button
              onClick={() => { setActiveMenu(null); onOpenCloudMigration(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <RefreshCw className="w-3.5 h-3.5 text-emerald-400" />
              <span>Microsoft 365 / Google / EWS / IMAP</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenDeltaSync(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <RefreshCw className="w-3.5 h-3.5 text-cyan-400" />
              <span>Incremental Delta Checkpoints</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenSplitter(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <Split className="w-3.5 h-3.5 text-purple-400" />
              <span>PST Auto-Split Threshold (20GB/45GB)</span>
            </button>
          </div>
        )}
      </div>

      {/* Forensics Menu */}
      <div className="relative">
        <button
          onClick={() => toggleMenu('forensics')}
          className={`px-2.5 py-1 rounded transition-colors ${
            activeMenu === 'forensics' ? 'bg-slate-700 text-white' : 'hover:bg-slate-800 text-slate-200'
          }`}
        >
          Forensics & eDiscovery
        </button>
        {activeMenu === 'forensics' && (
          <div className="absolute left-0 top-full mt-1 w-64 bg-slate-800 border border-slate-700 rounded-md shadow-2xl py-1 z-50 text-slate-200">
            <button
              onClick={() => { setActiveMenu(null); onOpenForensics(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <Shield className="w-3.5 h-3.5 text-amber-400" />
              <span>Chain of Custody & Cryptographic Hash</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenForensics(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <FileText className="w-3.5 h-3.5 text-blue-400" />
              <span>Bates Stamping Sequencer</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenExportReport(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <FileText className="w-3.5 h-3.5 text-emerald-400" />
              <span>Export Compliance Audit Report (PDF / CSV)</span>
            </button>
            <button
              onClick={() => { setActiveMenu(null); onOpenSecurityAudit(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <Shield className="w-3.5 h-3.5 text-purple-400" />
              <span>Real-Time Security Audit Viewer (ISO 27001)</span>
            </button>
          </div>
        )}
      </div>

      {/* Legacy Formats Menu */}
      <div className="relative">
        <button
          onClick={() => toggleMenu('legacy')}
          className={`px-2.5 py-1 rounded transition-colors ${
            activeMenu === 'legacy' ? 'bg-slate-700 text-white' : 'hover:bg-slate-800 text-slate-200'
          }`}
        >
          Legacy Formats
        </button>
        {activeMenu === 'legacy' && (
          <div className="absolute left-0 top-full mt-1 w-60 bg-slate-800 border border-slate-700 rounded-md shadow-2xl py-1 z-50 text-slate-200">
            <button
              onClick={() => { setActiveMenu(null); onOpenLegacyFormats(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <Layers className="w-3.5 h-3.5 text-indigo-400" />
              <span>DBX, NSF, EMLX, TBB, MBX Sniffer</span>
            </button>
          </div>
        )}
      </div>

      {/* Rust Engine Menu */}
      <div className="relative">
        <button
          onClick={() => toggleMenu('rust')}
          className={`px-2.5 py-1 rounded transition-colors ${
            activeMenu === 'rust' ? 'bg-slate-700 text-white' : 'hover:bg-slate-800 text-slate-200'
          }`}
        >
          Rust Engine
        </button>
        {activeMenu === 'rust' && (
          <div className="absolute left-0 top-full mt-1 w-64 bg-slate-800 border border-slate-700 rounded-md shadow-2xl py-1 z-50 text-slate-200">
            <button
              onClick={() => { setActiveMenu(null); onOpenRustCodebase(); }}
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center space-x-2"
            >
              <Code className="w-3.5 h-3.5 text-orange-400" />
              <span>Browse Rust Crate Sources & Architecture</span>
            </button>
          </div>
        )}
      </div>

      {/* Help Menu */}
      <div className="relative">
        <button
          onClick={() => toggleMenu('help')}
          className={`px-2.5 py-1 rounded transition-colors ${
            activeMenu === 'help' ? 'bg-slate-700 text-white' : 'hover:bg-slate-800 text-slate-200'
          }`}
        >
          Help
        </button>
        {activeMenu === 'help' && (
          <div className="absolute left-0 top-full mt-1 w-64 bg-slate-800 border border-slate-700 rounded-md shadow-2xl py-1 z-50 text-slate-200">
            <a
              href="https://github.com/adpaolucci/mail2mail/"
              target="_blank"
              rel="noreferrer"
              className="w-full text-left px-3 py-1.5 hover:bg-blue-600 hover:text-white text-xs flex items-center justify-between"
            >
              <span className="flex items-center space-x-2">
                <ExternalLink className="w-3.5 h-3.5 text-slate-400" />
                <span>GitHub Repository (adpaolucci/mail2mail)</span>
              </span>
            </a>
            <div className="h-px bg-slate-700 my-1" />
            <div className="px-3 py-1.5 text-[11px] text-slate-400">
              Mail2Mail Studio v3.2.0<br />
              Direct MS-PST / MAPI Architecture in Rust
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
