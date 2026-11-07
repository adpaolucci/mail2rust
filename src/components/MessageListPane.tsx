import React, { useState, useRef, useEffect } from 'react';
import {
  Search,
  Paperclip,
  Flag,
  AlertCircle,
  ArrowUpDown,
  Mail,
  MailOpen,
  Trash2,
  CheckSquare,
  Square,
  MinusSquare,
  AlertTriangle,
  X,
  ShieldAlert,
} from 'lucide-react';
import { EmailMessage } from '../types/mail';

interface MessageListPaneProps {
  messages: EmailMessage[];
  selectedMessageId: string | null;
  onSelectMessage: (id: string) => void;
  folderName: string;
  searchQuery: string;
  onSearchChange: (q: string) => void;
  checkedMessageIds: string[];
  onToggleCheckMessage: (id: string) => void;
  onSelectAllVisible: (ids: string[]) => void;
  onClearCheckedMessages: () => void;
  onBulkDelete: (ids: string[], allowLegalHoldOverride: boolean) => void;
}

export const MessageListPane: React.FC<MessageListPaneProps> = ({
  messages,
  selectedMessageId,
  onSelectMessage,
  folderName,
  searchQuery,
  onSearchChange,
  checkedMessageIds,
  onToggleCheckMessage,
  onSelectAllVisible,
  onClearCheckedMessages,
  onBulkDelete,
}) => {
  const [sortField, setSortField] = useState<'date' | 'sender' | 'subject' | 'size'>('date');
  const [sortAsc, setSortAsc] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [overrideLegalHold, setOverrideLegalHold] = useState(false);

  const masterCheckboxRef = useRef<HTMLInputElement>(null);

  const handleSort = (field: 'date' | 'sender' | 'subject' | 'size') => {
    if (sortField === field) {
      setSortAsc(!sortAsc);
    } else {
      setSortField(field);
    }
  };

  const sortedMessages = [...messages].sort((a, b) => {
    let cmp = 0;
    if (sortField === 'date') {
      cmp = new Date(a.date).getTime() - new Date(b.date).getTime();
    } else if (sortField === 'sender') {
      cmp = a.from.name.localeCompare(b.from.name);
    } else if (sortField === 'subject') {
      cmp = a.subject.localeCompare(b.subject);
    } else if (sortField === 'size') {
      cmp = a.sizeBytes - b.sizeBytes;
    }
    return sortAsc ? cmp : -cmp;
  });

  const visibleIds = sortedMessages.map((m) => m.id);
  const selectedVisibleCount = visibleIds.filter((id) => checkedMessageIds.includes(id)).length;
  const isAllVisibleChecked = visibleIds.length > 0 && selectedVisibleCount === visibleIds.length;
  const isPartiallyChecked = selectedVisibleCount > 0 && !isAllVisibleChecked;

  useEffect(() => {
    if (masterCheckboxRef.current) {
      masterCheckboxRef.current.indeterminate = isPartiallyChecked;
    }
  }, [isPartiallyChecked]);

  const handleToggleMasterCheckbox = () => {
    if (isAllVisibleChecked) {
      onClearCheckedMessages();
    } else {
      onSelectAllVisible(visibleIds);
    }
  };

  // Inspect checked items for legal hold status
  const checkedMessages = messages.filter((m) => checkedMessageIds.includes(m.id));
  const legalHoldItems = checkedMessages.filter((m) => m.legalHold);
  const hasLegalHold = legalHoldItems.length > 0;

  const handleConfirmDelete = () => {
    onBulkDelete(checkedMessageIds, overrideLegalHold);
    setShowDeleteConfirm(false);
    setOverrideLegalHold(false);
  };

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const formatDate = (dateStr: string) => {
    const d = new Date(dateStr);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  };

  return (
    <div className="w-96 min-w-[320px] max-w-[440px] bg-slate-900 border-r border-slate-700/80 flex flex-col h-full select-none text-xs">
      {/* Pane Top Header */}
      <div className="p-2.5 border-b border-slate-800 flex items-center justify-between bg-slate-850">
        <div className="flex items-center space-x-2">
          {/* Master Select All Checkbox */}
          <input
            ref={masterCheckboxRef}
            type="checkbox"
            checked={isAllVisibleChecked}
            onChange={handleToggleMasterCheckbox}
            className="w-3.5 h-3.5 rounded border-slate-600 bg-slate-800 text-blue-600 focus:ring-0 focus:ring-offset-0 cursor-pointer accent-blue-500"
            title={isAllVisibleChecked ? 'Deselect all visible' : 'Select all visible'}
          />
          <div>
            <h2 className="font-semibold text-slate-100 text-sm tracking-tight">{folderName}</h2>
            <span className="text-[11px] text-slate-400 font-mono">
              {messages.length} {messages.length === 1 ? 'item' : 'items'}
              {checkedMessageIds.length > 0 && ` • ${checkedMessageIds.length} checked`}
            </span>
          </div>
        </div>

        {/* Quick Sort Dropdown */}
        <div className="flex items-center space-x-1">
          <button
            onClick={() => handleSort('date')}
            className="flex items-center space-x-1 px-2 py-1 rounded bg-slate-800 hover:bg-slate-750 text-slate-300 border border-slate-700/80 text-[11px] transition-colors"
            title="Sort by Date"
          >
            <span>Date</span>
            <ArrowUpDown className="w-3 h-3 text-slate-400" />
          </button>
        </div>
      </div>

      {/* Bulk Action Toolbar - Appears when 1+ messages are checked */}
      {checkedMessageIds.length > 0 && (
        <div className="p-2 bg-blue-950/40 border-b border-blue-900/60 flex items-center justify-between animate-fadeIn">
          <div className="flex items-center space-x-2">
            <span className="px-1.5 py-0.5 rounded bg-blue-600/30 text-blue-300 font-medium text-[11px] border border-blue-500/30">
              {checkedMessageIds.length} selected
            </span>
            <button
              onClick={handleToggleMasterCheckbox}
              className="text-[11px] text-blue-400 hover:text-blue-200 underline"
            >
              {isAllVisibleChecked ? 'Deselect All' : `Select All (${visibleIds.length})`}
            </button>
          </div>

          <div className="flex items-center space-x-1.5">
            <button
              onClick={() => setShowDeleteConfirm(true)}
              className="flex items-center space-x-1 px-2.5 py-1 rounded bg-rose-600 hover:bg-rose-500 text-white font-medium text-[11px] shadow-sm transition-colors cursor-pointer"
            >
              <Trash2 className="w-3 h-3" />
              <span>Delete ({checkedMessageIds.length})</span>
            </button>
            <button
              onClick={onClearCheckedMessages}
              className="p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-800"
              title="Clear selection"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      )}

      {/* Confirmation Overlay for Bulk Delete */}
      {showDeleteConfirm && (
        <div className="p-3 bg-rose-950/90 border-b border-rose-800/80 text-rose-100 flex flex-col space-y-2 animate-fadeIn">
          <div className="flex items-start space-x-2">
            <AlertTriangle className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
            <div className="flex-1">
              <p className="font-semibold text-xs text-white">
                Delete {checkedMessageIds.length} {checkedMessageIds.length === 1 ? 'Message' : 'Messages'}?
              </p>
              <p className="text-[11px] text-rose-200 mt-0.5">
                This will remove the selected messages from the global mailbox and update folder metrics.
              </p>

              {hasLegalHold && (
                <div className="mt-2 p-2 rounded bg-amber-950/80 border border-amber-600/60 text-amber-200 text-[10px] space-y-1.5">
                  <div className="flex items-center space-x-1 font-semibold text-amber-300">
                    <ShieldAlert className="w-3.5 h-3.5" />
                    <span>Statutory Legal Hold Notice (FRCP Rule 37(e))</span>
                  </div>
                  <p>
                    {legalHoldItems.length} of the selected messages are flagged with active legal holds. By default, they will be protected from deletion.
                  </p>
                  <label className="flex items-center space-x-1.5 cursor-pointer text-amber-100 mt-1">
                    <input
                      type="checkbox"
                      checked={overrideLegalHold}
                      onChange={(e) => setOverrideLegalHold(e.target.checked)}
                      className="w-3 h-3 rounded border-amber-600 bg-amber-900 text-rose-500 accent-rose-500"
                    />
                    <span>Force delete legal hold items (Not Recommended)</span>
                  </label>
                </div>
              )}
            </div>
          </div>

          <div className="flex items-center justify-end space-x-2 pt-1">
            <button
              onClick={() => {
                setShowDeleteConfirm(false);
                setOverrideLegalHold(false);
              }}
              className="px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-200 text-[11px] border border-slate-700"
            >
              Cancel
            </button>
            <button
              onClick={handleConfirmDelete}
              className="px-3 py-1 rounded bg-rose-600 hover:bg-rose-500 text-white font-medium text-[11px] shadow-sm flex items-center space-x-1 cursor-pointer"
            >
              <Trash2 className="w-3 h-3" />
              <span>Confirm Delete</span>
            </button>
          </div>
        </div>
      )}

      {/* Instant Filter Search Bar */}
      <div className="p-2 border-b border-slate-800/80 bg-slate-900/50">
        <div className="relative">
          <Search className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-1/2 -translate-y-1/2" />
          <input
            type="text"
            placeholder="Search sender, subject, body..."
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            className="w-full bg-slate-800 text-slate-100 placeholder-slate-400 text-xs pl-8 pr-3 py-1.5 rounded border border-slate-700 focus:outline-none focus:border-blue-500"
          />
        </div>
      </div>

      {/* Messages List View */}
      <div className="flex-1 overflow-y-auto divide-y divide-slate-800/60">
        {sortedMessages.length === 0 ? (
          <div className="p-8 text-center text-slate-400">
            <Mail className="w-8 h-8 text-slate-600 mx-auto mb-2" />
            <p className="font-medium text-slate-300">No messages match filter</p>
            <p className="text-[11px] mt-1 text-slate-500">Try adjusting your date or search parameters</p>
          </div>
        ) : (
          sortedMessages.map((msg) => {
            const isSelected = selectedMessageId === msg.id;
            const isChecked = checkedMessageIds.includes(msg.id);

            return (
              <div
                key={msg.id}
                onClick={() => onSelectMessage(msg.id)}
                className={`p-3 cursor-pointer transition-colors relative flex items-start space-x-2.5 ${
                  isChecked
                    ? 'bg-blue-950/30 border-l-2 border-blue-400'
                    : isSelected
                    ? 'bg-blue-600/25 border-l-2 border-blue-500'
                    : msg.isRead
                    ? 'hover:bg-slate-800/50 text-slate-300'
                    : 'bg-slate-800/30 hover:bg-slate-800/70 text-slate-100 font-medium'
                }`}
              >
                {/* Checkbox for message */}
                <div
                  className="pt-0.5 shrink-0"
                  onClick={(e) => e.stopPropagation()}
                >
                  <input
                    type="checkbox"
                    checked={isChecked}
                    onChange={() => onToggleCheckMessage(msg.id)}
                    aria-label={`Select message ${msg.subject}`}
                    className="w-3.5 h-3.5 rounded border-slate-600 bg-slate-800 text-blue-600 focus:ring-0 focus:ring-offset-0 cursor-pointer accent-blue-500"
                  />
                </div>

                {/* Message Content Container */}
                <div className="flex-1 min-w-0">
                  {/* Top Row: Sender + Indicators + Date */}
                  <div className="flex items-center justify-between mb-1">
                    <div className="flex items-center space-x-1.5 min-w-0">
                      {msg.isRead ? (
                        <MailOpen className="w-3 h-3 text-slate-400 shrink-0" />
                      ) : (
                        <div className="w-2 h-2 rounded-full bg-blue-500 shrink-0" />
                      )}
                      <span className={`truncate text-xs ${!msg.isRead ? 'font-semibold text-white' : 'text-slate-200'}`}>
                        {msg.from.name}
                      </span>
                    </div>

                    <div className="flex items-center space-x-2 shrink-0 ml-2">
                      {msg.attachments.length > 0 && (
                        <Paperclip className="w-3 h-3 text-slate-400" />
                      )}
                      {msg.importance === 'high' && (
                        <AlertCircle className="w-3 h-3 text-rose-400" />
                      )}
                      {msg.isFlagged && (
                        <Flag className="w-3 h-3 text-amber-400 fill-amber-400" />
                      )}
                      {msg.legalHold && (
                        <span className="w-1.5 h-1.5 rounded-full bg-amber-400" title="Legal Hold Active" />
                      )}
                      <span className="text-[11px] text-slate-400 font-mono">
                        {formatDate(msg.date)}
                      </span>
                    </div>
                  </div>

                  {/* Subject */}
                  <h4 className="text-xs text-slate-200 font-medium truncate mb-1">
                    {msg.subject || '(No Subject)'}
                  </h4>

                  {/* Snippet */}
                  <p className="text-[11px] text-slate-400 line-clamp-2 leading-relaxed">
                    {msg.bodyText.replace(/\n+/g, ' ')}
                  </p>

                  {/* Bottom Metadata Badges */}
                  <div className="mt-2 flex items-center justify-between text-[10px] text-slate-400 font-mono">
                    <div className="flex items-center space-x-1.5">
                      <span className="px-1.5 py-0.2 rounded bg-slate-800 border border-slate-700/60 text-slate-300">
                        {msg.sourceFormat}
                      </span>
                      {msg.batesNumber && (
                        <span className="px-1.5 py-0.2 rounded bg-amber-950/70 border border-amber-700/60 text-amber-300">
                          {msg.batesNumber}
                        </span>
                      )}
                      {msg.legalHold && (
                        <span className="px-1.5 py-0.2 rounded bg-amber-900/60 border border-amber-600/60 text-amber-300 font-sans">
                          HOLD
                        </span>
                      )}
                    </div>
                    <span>{formatSize(msg.sizeBytes)}</span>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
