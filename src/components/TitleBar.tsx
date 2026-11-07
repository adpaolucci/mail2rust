import React from 'react';
import { Mail, Minus, Square, X, ShieldCheck, Cpu, HardDrive } from 'lucide-react';

interface TitleBarProps {
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  onOpenRustWorkspace: () => void;
}

export const TitleBar: React.FC<TitleBarProps> = ({
  onMinimize,
  onMaximize,
  onClose,
  onOpenRustWorkspace,
}) => {
  return (
    <div className="h-9 bg-slate-900 border-b border-slate-700/80 flex items-center justify-between px-3 select-none text-xs text-slate-300 font-sans">
      {/* Left: Window Icon + Title */}
      <div className="flex items-center space-x-2.5">
        <div className="w-5 h-5 rounded bg-blue-600 flex items-center justify-center text-white shadow-sm">
          <Mail className="w-3.5 h-3.5" />
        </div>
        <span className="font-semibold text-slate-100 tracking-tight">Mail2Mail Studio Desktop</span>
        <span className="text-slate-500 font-mono text-[11px]">v3.2.0</span>
        <span className="hidden sm:inline-block text-slate-500">|</span>
        <span className="hidden md:inline-block text-slate-400 text-[11px]">
          [Project Titan: Enterprise Migration & Forensic PST Suite]
        </span>
      </div>

      {/* Middle: Engine Badges */}
      <div className="hidden lg:flex items-center space-x-3 text-[11px]">
        <button
          onClick={onOpenRustWorkspace}
          className="flex items-center space-x-1.5 px-2 py-0.5 rounded bg-orange-950/70 border border-orange-600/50 text-orange-300 hover:bg-orange-900/60 transition-colors"
          title="Click to view native Rust workspace codebase"
        >
          <Cpu className="w-3 h-3 text-orange-400" />
          <span className="font-mono">Rust Engine (Direct MAPI)</span>
        </button>
        <div className="flex items-center space-x-1 px-2 py-0.5 rounded bg-emerald-950/60 border border-emerald-600/40 text-emerald-300">
          <ShieldCheck className="w-3 h-3 text-emerald-400" />
          <span>Outlook Independent</span>
        </div>
        <div className="flex items-center space-x-1 px-2 py-0.5 rounded bg-blue-950/60 border border-blue-600/40 text-blue-300">
          <HardDrive className="w-3 h-3 text-blue-400" />
          <span>Unicode PST [MS-PST]</span>
        </div>
      </div>

      {/* Right: Windows Window Controls */}
      <div className="flex items-center -mr-2">
        <button
          aria-label="Minimize"
          onClick={onMinimize}
          className="w-10 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
        >
          <Minus className="w-3.5 h-3.5" />
        </button>
        <button
          aria-label="Maximize"
          onClick={onMaximize}
          className="w-10 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
        >
          <Square className="w-3 h-3" />
        </button>
        <button
          aria-label="Close"
          onClick={onClose}
          className="w-11 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-red-600 transition-colors"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );
};
