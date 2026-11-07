import React, { useState } from 'react';
import {
  Code,
  X,
  Download,
  Copy,
  Check,
  FileCode,
  Folder,
  Terminal,
  Cpu,
  Layers,
} from 'lucide-react';
import { RUST_WORKSPACE_FILES, RustSourceFile } from '../data/rustCodebase';
import JSZip from 'jszip';

interface RustCodebaseModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const RustCodebaseModal: React.FC<RustCodebaseModalProps> = ({
  isOpen,
  onClose,
}) => {
  const [selectedFile, setSelectedFile] = useState<RustSourceFile>(RUST_WORKSPACE_FILES[0]);
  const [copied, setCopied] = useState(false);
  const [isExporting, setIsExporting] = useState(false);

  if (!isOpen) return null;

  const handleCopyCode = () => {
    navigator.clipboard.writeText(selectedFile.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownloadFullZip = async () => {
    setIsExporting(true);
    const zip = new JSZip();

    RUST_WORKSPACE_FILES.forEach((f) => {
      zip.file(f.path, f.content);
    });

    // Add README.md for the Rust crate
    zip.file(
      'README.md',
      `# Mail2Mail Studio - Rust Native Workspace
Paired with: https://github.com/adpaolucci/mail2mail/

## Architecture
- \`crates/mail2mail-core\`: Streaming MIME, DBX, NSF, and Maildir parsers.
- \`crates/mail2mail-pst\`: Direct MS-PST writer (NDB layer, B-Trees, LTP Property Context, Table Context) with zero Outlook / MAPI dependency.
- \`crates/mail2mail-cloud\`: Async Tokio workers for Microsoft 365 Graph, Exchange EWS, Google Workspace REST, and IMAP4.
- \`crates/mail2mail-split\`: 20 GB / 45 GB multi-volume auto-splitter with thread clustering.
- \`crates/mail2mail-desktop\`: Tauri v2 Windows desktop application shell.

## Build Instructions
\`\`\`bash
# Build core CLI engine
cargo build --release

# Build Windows Desktop executable with Tauri v2
cargo tauri build
\`\`\`
`
    );

    const content = await zip.generateAsync({ type: 'blob' });
    const url = URL.createObjectURL(content);
    const a = document.createElement('a');
    a.href = url;
    a.download = `mail2mail_rust_workspace_${Date.now()}.zip`;
    a.click();
    URL.revokeObjectURL(url);
    setIsExporting(false);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-4xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-orange-600/30 border border-orange-500/40 flex items-center justify-center text-orange-400">
              <Terminal className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Rust Native Architecture & Workspace</h2>
              <p className="text-[11px] text-slate-400">
                Complete Rust crate workspace for mail2mail &bull; Direct MS-PST writer, async cloud connectors & Tauri v2 desktop
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

        {/* Content Pane */}
        <div className="flex-1 flex overflow-hidden">
          {/* File Explorer Sidebar */}
          <div className="w-72 bg-slate-950 border-r border-slate-800 flex flex-col p-2 select-none">
            <div className="text-[10px] text-slate-500 font-mono uppercase px-2 mb-2 font-bold tracking-wider">
              Workspace Crates
            </div>

            <div className="flex-1 overflow-y-auto space-y-1">
              {RUST_WORKSPACE_FILES.map((file) => {
                const isSelected = selectedFile.path === file.path;
                return (
                  <button
                    key={file.path}
                    onClick={() => setSelectedFile(file)}
                    className={`w-full text-left p-2 rounded flex items-start space-x-2 transition-colors ${
                      isSelected
                        ? 'bg-orange-600/25 border-l-2 border-orange-500 text-white font-medium'
                        : 'hover:bg-slate-900 text-slate-400 hover:text-slate-200'
                    }`}
                  >
                    <FileCode className="w-3.5 h-3.5 text-orange-400 shrink-0 mt-0.5" />
                    <div className="min-w-0">
                      <div className="font-mono text-[11px] truncate">{file.path}</div>
                      <div className="text-[10px] text-slate-500 truncate">{file.crate}</div>
                    </div>
                  </button>
                );
              })}
            </div>

            <div className="pt-2 border-t border-slate-800">
              <button
                onClick={handleDownloadFullZip}
                disabled={isExporting}
                className="w-full py-2 px-3 rounded-lg bg-orange-600 hover:bg-orange-500 text-white font-medium flex items-center justify-center space-x-1.5 transition-colors shadow-sm disabled:opacity-50"
              >
                <Download className="w-3.5 h-3.5" />
                <span>{isExporting ? 'Packing...' : 'Download Full Rust Workspace (.zip)'}</span>
              </button>
            </div>
          </div>

          {/* Code Viewer */}
          <div className="flex-1 flex flex-col bg-slate-900 overflow-hidden">
            <div className="p-3 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
              <div>
                <h3 className="font-mono text-slate-200 text-xs font-semibold">{selectedFile.path}</h3>
                <p className="text-[11px] text-slate-400">{selectedFile.description}</p>
              </div>

              <button
                onClick={handleCopyCode}
                className="flex items-center space-x-1 px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-750 border border-slate-700 text-slate-300 text-xs"
              >
                {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                <span>{copied ? 'Copied' : 'Copy Code'}</span>
              </button>
            </div>

            <div className="flex-1 overflow-auto p-4 bg-slate-950 font-mono text-[11px] text-slate-300 leading-relaxed">
              <pre className="select-text">{selectedFile.content}</pre>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="p-3.5 border-t border-slate-800 bg-slate-850 flex items-center justify-between text-[11px] text-slate-400">
          <div className="flex items-center space-x-2 font-mono">
            <span>Pairs with: https://github.com/adpaolucci/mail2mail/</span>
          </div>

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
