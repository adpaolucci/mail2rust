import React, { useState } from 'react';
import {
  Filter,
  X,
  Calendar,
  User,
  Paperclip,
  Check,
  RotateCcw,
  Sliders,
  Shield,
  FileCheck,
} from 'lucide-react';
import { FilterCriteria, EmailMessage } from '../types/mail';
import { filterMessages } from '../utils/filterEngine';

interface FilterModalProps {
  isOpen: boolean;
  onClose: () => void;
  criteria: FilterCriteria;
  onApplyCriteria: (c: FilterCriteria) => void;
  allMessages: EmailMessage[];
}

export const FilterModal: React.FC<FilterModalProps> = ({
  isOpen,
  onClose,
  criteria,
  onApplyCriteria,
  allMessages,
}) => {
  const [localCriteria, setLocalCriteria] = useState<FilterCriteria>({ ...criteria });

  if (!isOpen) return null;

  // Real-time matched count preview
  const matched = filterMessages(allMessages, localCriteria);
  const matchedSize = matched.reduce((sum, m) => sum + m.sizeBytes, 0);

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const handleApplyPreset = (preset: 'all' | 'last7' | 'last30' | 'ytd') => {
    const today = new Date('2026-11-06T00:00:00Z');
    if (preset === 'all') {
      setLocalCriteria(prev => ({ ...prev, startDate: null, endDate: null }));
    } else if (preset === 'last7') {
      const d = new Date(today);
      d.setDate(d.getDate() - 7);
      setLocalCriteria(prev => ({
        ...prev,
        startDate: d.toISOString().split('T')[0],
        endDate: today.toISOString().split('T')[0],
      }));
    } else if (preset === 'last30') {
      const d = new Date(today);
      d.setDate(d.getDate() - 30);
      setLocalCriteria(prev => ({
        ...prev,
        startDate: d.toISOString().split('T')[0],
        endDate: today.toISOString().split('T')[0],
      }));
    } else if (preset === 'ytd') {
      setLocalCriteria(prev => ({
        ...prev,
        startDate: '2026-01-01',
        endDate: today.toISOString().split('T')[0],
      }));
    }
  };

  const handleReset = () => {
    const emptyCriteria: FilterCriteria = {
      startDate: null,
      endDate: null,
      senderSearch: '',
      recipientSearch: '',
      subjectSearch: '',
      hasAttachments: null,
      minSizeBytes: null,
      maxSizeBytes: null,
      importance: 'all',
      readStatus: 'all',
      flaggedOnly: false,
      legalHoldOnly: false,
    };
    setLocalCriteria(emptyCriteria);
  };

  const handleSave = () => {
    onApplyCriteria(localCriteria);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-2xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-amber-600/30 border border-amber-500/40 flex items-center justify-center text-amber-400">
              <Filter className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Date, Sender & Metadata Filtering</h2>
              <p className="text-[11px] text-slate-400">
                Filter emails before exporting to PST, splitting volumes, or cloud migration
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
          {/* Date Filter Section */}
          <div className="bg-slate-800/50 border border-slate-700/80 rounded-lg p-3 space-y-3">
            <div className="flex items-center justify-between">
              <span className="font-semibold text-slate-200 flex items-center space-x-1.5">
                <Calendar className="w-4 h-4 text-blue-400" />
                <span>Date Range (Received / Sent)</span>
              </span>

              {/* Date Presets */}
              <div className="flex items-center space-x-1">
                <button
                  onClick={() => handleApplyPreset('last7')}
                  className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px]"
                >
                  Last 7 Days
                </button>
                <button
                  onClick={() => handleApplyPreset('last30')}
                  className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px]"
                >
                  Last 30 Days
                </button>
                <button
                  onClick={() => handleApplyPreset('ytd')}
                  className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px]"
                >
                  2026 YTD
                </button>
                <button
                  onClick={() => handleApplyPreset('all')}
                  className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px]"
                >
                  All Dates
                </button>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-slate-400 text-[11px] mb-1">Start Date (From)</label>
                <input
                  type="date"
                  value={localCriteria.startDate || ''}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, startDate: e.target.value || null }))}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
                />
              </div>
              <div>
                <label className="block text-slate-400 text-[11px] mb-1">End Date (To)</label>
                <input
                  type="date"
                  value={localCriteria.endDate || ''}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, endDate: e.target.value || null }))}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
                />
              </div>
            </div>
          </div>

          {/* Sender & Recipient Filtering */}
          <div className="bg-slate-800/50 border border-slate-700/80 rounded-lg p-3 space-y-3">
            <span className="font-semibold text-slate-200 flex items-center space-x-1.5">
              <User className="w-4 h-4 text-emerald-400" />
              <span>Sender & Recipient Query</span>
            </span>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-slate-400 text-[11px] mb-1">Sender (From email or name)</label>
                <input
                  type="text"
                  placeholder="e.g. evelyn, @titan.corp"
                  value={localCriteria.senderSearch || ''}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, senderSearch: e.target.value }))}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
                />
              </div>
              <div>
                <label className="block text-slate-400 text-[11px] mb-1">Recipient (To / Cc)</label>
                <input
                  type="text"
                  placeholder="e.g. adpaolucci, legal@"
                  value={localCriteria.recipientSearch || ''}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, recipientSearch: e.target.value }))}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
                />
              </div>
            </div>
          </div>

          {/* Subject & Metadata Filters */}
          <div className="bg-slate-800/50 border border-slate-700/80 rounded-lg p-3 space-y-3">
            <span className="font-semibold text-slate-200 flex items-center space-x-1.5">
              <Sliders className="w-4 h-4 text-purple-400" />
              <span>Metadata & Flags</span>
            </span>

            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="block text-slate-400 text-[11px] mb-1">Attachment Status</label>
                <select
                  value={localCriteria.hasAttachments === null ? 'any' : localCriteria.hasAttachments ? 'has' : 'none'}
                  onChange={(e) => {
                    const val = e.target.value;
                    setLocalCriteria(prev => ({
                      ...prev,
                      hasAttachments: val === 'any' ? null : val === 'has',
                    }));
                  }}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
                >
                  <option value="any">Any (With or without)</option>
                  <option value="has">Has Attachments Only</option>
                  <option value="none">No Attachments</option>
                </select>
              </div>

              <div>
                <label className="block text-slate-400 text-[11px] mb-1">Importance</label>
                <select
                  value={localCriteria.importance || 'all'}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, importance: e.target.value as any }))}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
                >
                  <option value="all">All Levels</option>
                  <option value="high">High Importance Only</option>
                  <option value="normal">Normal</option>
                  <option value="low">Low</option>
                </select>
              </div>

              <div>
                <label className="block text-slate-400 text-[11px] mb-1">Read / Unread</label>
                <select
                  value={localCriteria.readStatus || 'all'}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, readStatus: e.target.value as any }))}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100"
                >
                  <option value="all">All Messages</option>
                  <option value="unread">Unread Only</option>
                  <option value="read">Read Only</option>
                </select>
              </div>
            </div>

            {/* Checkbox Toggles */}
            <div className="flex items-center space-x-6 pt-2">
              <label className="flex items-center space-x-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={Boolean(localCriteria.flaggedOnly)}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, flaggedOnly: e.target.checked }))}
                  className="rounded bg-slate-800 border-slate-700 text-amber-500"
                />
                <span className="text-slate-300">Flagged Messages Only</span>
              </label>

              <label className="flex items-center space-x-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={Boolean(localCriteria.legalHoldOnly)}
                  onChange={(e) => setLocalCriteria(prev => ({ ...prev, legalHoldOnly: e.target.checked }))}
                  className="rounded bg-slate-800 border-slate-700 text-rose-500"
                />
                <span className="text-slate-300">Legal Hold Messages Only</span>
              </label>
            </div>
          </div>

          {/* Real-Time Match Counter Preview */}
          <div className="bg-slate-950 border border-slate-800 rounded-lg p-3 flex items-center justify-between text-xs font-mono">
            <span className="text-slate-400">Filter Simulation Match:</span>
            <span className="text-emerald-400 font-bold">
              {matched.length} of {allMessages.length} messages ({formatSize(matchedSize)})
            </span>
          </div>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <button
            onClick={handleReset}
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded border border-slate-700 hover:bg-slate-800 text-slate-400 hover:text-slate-200 transition-colors"
          >
            <RotateCcw className="w-3.5 h-3.5" />
            <span>Reset Filters</span>
          </button>

          <div className="flex items-center space-x-2">
            <button
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg border border-slate-700 hover:bg-slate-800 text-slate-300 transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              className="px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5"
            >
              <Check className="w-3.5 h-3.5" />
              <span>Apply Filters</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
