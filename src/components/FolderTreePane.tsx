import React from 'react';
import {
  Inbox,
  Send,
  FileEdit,
  Archive,
  Trash2,
  Calendar,
  Users,
  Folder,
  CheckSquare,
  Square,
  MinusSquare,
  PlusCircle,
  HardDrive,
} from 'lucide-react';
import { MailboxFolder } from '../types/mail';

interface FolderTreePaneProps {
  folders: MailboxFolder[];
  activeFolderId: string | null;
  onSelectFolder: (id: string) => void;
  onToggleFolderCheck: (id: string) => void;
  onSelectAllFolders: () => void;
  onClearFolderSelection: () => void;
  onAddCustomFolder?: () => void;
}

export const FolderTreePane: React.FC<FolderTreePaneProps> = ({
  folders,
  activeFolderId,
  onSelectFolder,
  onToggleFolderCheck,
  onSelectAllFolders,
  onClearFolderSelection,
  onAddCustomFolder,
}) => {
  const allSelected = folders.every(f => f.selected);
  const someSelected = folders.some(f => f.selected) && !allSelected;

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const getFolderIcon = (type: MailboxFolder['folderType']) => {
    switch (type) {
      case 'inbox':
        return <Inbox className="w-4 h-4 text-blue-400 shrink-0" />;
      case 'sent':
        return <Send className="w-4 h-4 text-emerald-400 shrink-0" />;
      case 'drafts':
        return <FileEdit className="w-4 h-4 text-amber-400 shrink-0" />;
      case 'archive':
        return <Archive className="w-4 h-4 text-purple-400 shrink-0" />;
      case 'trash':
        return <Trash2 className="w-4 h-4 text-rose-400 shrink-0" />;
      case 'calendar':
        return <Calendar className="w-4 h-4 text-cyan-400 shrink-0" />;
      case 'contacts':
        return <Users className="w-4 h-4 text-teal-400 shrink-0" />;
      default:
        return <Folder className="w-4 h-4 text-indigo-400 shrink-0" />;
    }
  };

  return (
    <div className="w-72 bg-slate-900 border-r border-slate-700/80 flex flex-col h-full select-none text-xs">
      {/* Header */}
      <div className="p-2.5 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <HardDrive className="w-4 h-4 text-blue-400" />
          <span className="font-semibold text-slate-200">Mailbox Hierarchy</span>
        </div>
        <div className="flex items-center space-x-1">
          <button
            onClick={allSelected ? onClearFolderSelection : onSelectAllFolders}
            className="p-1 text-slate-400 hover:text-slate-100 hover:bg-slate-800 rounded transition-colors"
            title={allSelected ? 'Deselect All' : 'Select All'}
          >
            {allSelected ? (
              <CheckSquare className="w-4 h-4 text-blue-400" />
            ) : someSelected ? (
              <MinusSquare className="w-4 h-4 text-blue-400" />
            ) : (
              <Square className="w-4 h-4 text-slate-400" />
            )}
          </button>
        </div>
      </div>

      {/* Selective Tree Navigation Info */}
      <div className="px-3 py-1.5 bg-slate-950/40 border-b border-slate-800/60 text-[11px] text-slate-400 flex items-center justify-between">
        <span>Check folders to include in PST / Migration</span>
        <span className="font-mono text-slate-300">
          {folders.filter(f => f.selected).length}/{folders.length} selected
        </span>
      </div>

      {/* Folder List */}
      <div className="flex-1 overflow-y-auto py-1">
        {folders.map(folder => {
          const isActive = activeFolderId === folder.id;
          const isChild = folder.parentId !== null;

          return (
            <div
              key={folder.id}
              className={`group flex items-center justify-between px-3 py-1.5 cursor-pointer transition-colors ${
                isActive
                  ? 'bg-blue-600/20 text-white font-medium border-l-2 border-blue-500'
                  : 'text-slate-300 hover:bg-slate-800/70 hover:text-white'
              } ${isChild ? 'pl-7' : ''}`}
              onClick={() => onSelectFolder(folder.id)}
            >
              <div className="flex items-center space-x-2 min-w-0">
                {/* Checkbox for selective migration */}
                <button
                  type="button"
                  aria-label={`Toggle inclusion of ${folder.name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onToggleFolderCheck(folder.id);
                  }}
                  className="p-0.5 text-slate-400 hover:text-blue-400"
                >
                  {folder.selected ? (
                    <CheckSquare className="w-3.5 h-3.5 text-blue-400" />
                  ) : (
                    <Square className="w-3.5 h-3.5 text-slate-500" />
                  )}
                </button>

                {getFolderIcon(folder.folderType)}
                <span className="truncate">{folder.name}</span>
              </div>

              {/* Counts & Size */}
              <div className="flex items-center space-x-2 shrink-0 ml-2">
                {folder.unreadCount > 0 && (
                  <span className="px-1.5 py-0.2 rounded-full bg-blue-500 text-white text-[10px] font-bold">
                    {folder.unreadCount}
                  </span>
                )}
                <span className="text-[10px] text-slate-400 font-mono">
                  {folder.totalCount} ({formatSize(folder.sizeBytes)})
                </span>
              </div>
            </div>
          );
        })}
      </div>

      {/* Bottom Container Info */}
      <div className="p-2.5 border-t border-slate-800 bg-slate-950/60 text-[11px] text-slate-400">
        <div className="flex justify-between mb-1">
          <span>Target Container:</span>
          <span className="font-semibold text-slate-200">Unicode MS-PST</span>
        </div>
        <div className="flex justify-between text-slate-400">
          <span>Total Selected:</span>
          <span className="font-mono text-emerald-400">
            {formatSize(folders.filter(f => f.selected).reduce((acc, f) => acc + f.sizeBytes, 0))}
          </span>
        </div>
      </div>
    </div>
  );
};
