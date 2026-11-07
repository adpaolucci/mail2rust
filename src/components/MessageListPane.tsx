import React, { useState } from 'react';
import {
  Search,
  Paperclip,
  Flag,
  AlertCircle,
  Shield,
  FileCheck,
  ArrowUpDown,
  Mail,
  MailOpen,
  Calendar,
} from 'lucide-react';
import { EmailMessage } from '../types/mail';

interface MessageListPaneProps {
  messages: EmailMessage[];
  selectedMessageId: string | null;
  onSelectMessage: (id: string) => void;
  folderName: string;
  searchQuery: string;
  onSearchChange: (q: string) => void;
}

export const MessageListPane: React.FC<MessageListPaneProps> = ({
  messages,
  selectedMessageId,
  onSelectMessage,
  folderName,
  searchQuery,
  onSearchChange,
}) => {
  const [sortField, setSortField] = useState<'date' | 'sender' | 'subject' | 'size'>('date');
  const [sortAsc, setSortAsc] = useState(false);

  const handleSort = (field: 'date' | 'sender' | 'subject' | 'size') => {
    if (sortField === field) {
      setSortAsc(!sortAsc);
    } else {
      setSortField(field);
      setSortAsc(true);
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

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const formatDate = (dateStr: string) => {
    const d = new Date(dateStr);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  };

  return (
    <div className="w-96 min-w-[320px] max-w-[420px] bg-slate-850 bg-slate-900/90 border-r border-slate-700/80 flex flex-col h-full select-none text-xs">
      {/* Pane Top Header */}
      <div className="p-2.5 border-b border-slate-800 flex items-center justify-between">
        <div>
          <h2 className="font-semibold text-slate-100 text-sm tracking-tight">{folderName}</h2>
          <span className="text-[11px] text-slate-400 font-mono">
            {messages.length} {messages.length === 1 ? 'item' : 'items'}
          </span>
        </div>

        {/* Quick Sort Dropdown */}
        <div className="flex items-center space-x-1">
          <button
            onClick={() => handleSort('date')}
            className="flex items-center space-x-1 px-2 py-1 rounded bg-slate-800 hover:bg-slate-750 text-slate-300 border border-slate-700/80 text-[11px]"
            title="Sort by Date"
          >
            <span>Date</span>
            <ArrowUpDown className="w-3 h-3 text-slate-400" />
          </button>
        </div>
      </div>

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

            return (
              <div
                key={msg.id}
                onClick={() => onSelectMessage(msg.id)}
                className={`p-3 cursor-pointer transition-colors relative ${
                  isSelected
                    ? 'bg-blue-600/25 border-l-2 border-blue-500'
                    : msg.isRead
                    ? 'hover:bg-slate-800/50 text-slate-300'
                    : 'bg-slate-800/30 hover:bg-slate-800/70 text-slate-100 font-medium'
                }`}
              >
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
                  </div>
                  <span>{formatSize(msg.sizeBytes)}</span>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
