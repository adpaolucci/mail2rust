/**
 * Mail2Mail Studio Desktop
 * Native Windows Desktop Application paired with https://github.com/adpaolucci/mail2mail/
 * Direct PST Generation Without Outlook &bull; Multi-Cloud Migration &bull; eDiscovery Forensics &bull; Rust Architecture
 */

import React, { useState, useMemo } from 'react';
import { TitleBar } from './components/TitleBar';
import { MenuBar } from './components/MenuBar';
import { RibbonBar } from './components/RibbonBar';
import { FolderTreePane } from './components/FolderTreePane';
import { MessageListPane } from './components/MessageListPane';
import { MessagePreviewPane } from './components/MessagePreviewPane';
import { StatusBar } from './components/StatusBar';

import { DirectPstModal } from './components/DirectPstModal';
import { CloudMigrationModal } from './components/CloudMigrationModal';
import { FilterModal } from './components/FilterModal';
import { PstSplitModal } from './components/PstSplitModal';
import { DeltaSyncModal } from './components/DeltaSyncModal';
import { ForensicModal } from './components/ForensicModal';
import { LegacyFormatsModal } from './components/LegacyFormatsModal';
import { RustCodebaseModal } from './components/RustCodebaseModal';
import { ExportReportModal } from './components/ExportReportModal';
import { SecurityAuditViewer } from './components/SecurityAuditViewer';

import { INITIAL_FOLDERS, INITIAL_MESSAGES } from './data/seedData';
import { MailboxFolder, EmailMessage, FilterCriteria, SyncCheckpoint } from './types/mail';
import { filterMessages } from './utils/filterEngine';
import { bulkDeleteMessages, updateFolderStatsAfterDeletion } from './utils/messageOps';

export default function App() {
  const [folders, setFolders] = useState<MailboxFolder[]>(INITIAL_FOLDERS);
  const [messages, setMessages] = useState<EmailMessage[]>(INITIAL_MESSAGES);
  const [activeFolderId, setActiveFolderId] = useState<string>('inbox');
  const [selectedMessageId, setSelectedMessageId] = useState<string | null>(INITIAL_MESSAGES[0].id);
  const [searchQuery, setSearchQuery] = useState('');
  const [checkedMessageIds, setCheckedMessageIds] = useState<string[]>([]);

  // Filtering Criteria State
  const [filterCriteria, setFilterCriteria] = useState<FilterCriteria>({
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
  });

  // Checkpoints State for Incremental Delta Sync
  const [checkpoint, setCheckpoint] = useState<SyncCheckpoint | null>({
    id: 'chk_init_20261101',
    timestamp: '2026-11-01T08:00:00Z',
    sourceId: 'Project_Titan_Archive_Source',
    targetId: 'Master_Unicode_PST',
    migratedMessageIds: ['msg_001', 'msg_002', 'msg_005', 'msg_006', 'msg_011'],
    folderHighUids: { inbox: 2, sent: 0, archive: 1 },
    totalBytesMigrated: 1024 * 4800,
    totalMessagesMigrated: 5,
    status: 'completed',
  });

  // Modal Visibility States
  const [isDirectPstOpen, setIsDirectPstOpen] = useState(false);
  const [isCloudMigrationOpen, setIsCloudMigrationOpen] = useState(false);
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [isSplitterOpen, setIsSplitterOpen] = useState(false);
  const [isDeltaSyncOpen, setIsDeltaSyncOpen] = useState(false);
  const [isForensicsOpen, setIsForensicsOpen] = useState(false);
  const [isLegacyFormatsOpen, setIsLegacyFormatsOpen] = useState(false);
  const [isRustCodebaseOpen, setIsRustCodebaseOpen] = useState(false);
  const [isExportReportOpen, setIsExportReportOpen] = useState(false);
  const [isSecurityAuditOpen, setIsSecurityAuditOpen] = useState(false);

  // Folder Operations
  const handleToggleFolderCheck = (folderId: string) => {
    setFolders(prev =>
      prev.map(f => (f.id === folderId ? { ...f, selected: !f.selected } : f))
    );
  };

  const handleSelectAllFolders = () => {
    setFolders(prev => prev.map(f => ({ ...f, selected: true })));
  };

  const handleClearFolderSelection = () => {
    setFolders(prev => prev.map(f => ({ ...f, selected: false })));
  };

  const handleToggleLegalHold = (msgId: string) => {
    setMessages(prev =>
      prev.map(m => (m.id === msgId ? { ...m, legalHold: !m.legalHold } : m))
    );
  };

  const handleImportSampleLegacy = (newMsg: EmailMessage) => {
    setMessages(prev => [newMsg, ...prev]);
    setSelectedMessageId(newMsg.id);
    setActiveFolderId(newMsg.folderId);
  };

  const handleResetFilters = () => {
    setFilterCriteria({
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
    });
    setSearchQuery('');
  };

  // Multi-Select & Bulk Deletion Handlers
  const handleToggleCheckMessage = (id: string) => {
    setCheckedMessageIds(prev =>
      prev.includes(id) ? prev.filter(item => item !== id) : [...prev, id]
    );
  };

  const handleSelectAllVisibleMessages = (visibleIds: string[]) => {
    setCheckedMessageIds(prev => {
      const allChecked = visibleIds.length > 0 && visibleIds.every(id => prev.includes(id));
      if (allChecked) {
        return prev.filter(id => !visibleIds.includes(id));
      } else {
        return Array.from(new Set([...prev, ...visibleIds]));
      }
    });
  };

  const handleClearCheckedMessages = () => {
    setCheckedMessageIds([]);
  };

  const handleBulkDeleteMessages = (idsToDelete: string[], allowLegalHoldOverride: boolean) => {
    const result = bulkDeleteMessages(messages, idsToDelete, {
      allowLegalHoldDeletion: allowLegalHoldOverride,
    });

    // 1. Update messages state
    setMessages(result.remainingMessages);

    // 2. Update folder statistics (counts, unread, sizes)
    setFolders(prev => updateFolderStatsAfterDeletion(prev, result.deletedMessages));

    // 3. Clear deleted message IDs from checked list
    const deletedIdsSet = new Set(result.deletedMessages.map(m => m.id));
    setCheckedMessageIds(prev => prev.filter(id => !deletedIdsSet.has(id)));

    // 4. Update preview selection if current message was deleted
    if (selectedMessageId && deletedIdsSet.has(selectedMessageId)) {
      const remainingInActiveFolder = result.remainingMessages.filter(
        m => m.folderId === activeFolderId
      );
      setSelectedMessageId(
        remainingInActiveFolder.length > 0 ? remainingInActiveFolder[0].id : null
      );
    }
  };

  // Filter messages for current active folder and filter criteria
  const activeFolder = folders.find(f => f.id === activeFolderId) || null;

  const filteredFolderMessages = useMemo(() => {
    // 1. Filter by folder
    const inFolder = messages.filter(m => m.folderId === activeFolderId);

    // 2. Filter by search query
    let queryFiltered = inFolder;
    if (searchQuery.trim() !== '') {
      const q = searchQuery.toLowerCase().trim();
      queryFiltered = inFolder.filter(
        m =>
          m.subject.toLowerCase().includes(q) ||
          m.from.name.toLowerCase().includes(q) ||
          m.from.email.toLowerCase().includes(q) ||
          m.bodyText.toLowerCase().includes(q)
      );
    }

    // 3. Filter by criteria
    return filterMessages(queryFiltered, filterCriteria);
  }, [messages, activeFolderId, searchQuery, filterCriteria]);

  const allFilteredMessages = useMemo(() => {
    return filterMessages(messages, filterCriteria);
  }, [messages, filterCriteria]);

  const selectedMessage = messages.find(m => m.id === selectedMessageId) || null;

  return (
    <div className="flex flex-col h-screen w-screen bg-slate-900 text-slate-100 font-sans overflow-hidden select-none">
      {/* 1. Authentic Windows 11 Title Bar */}
      <TitleBar
        onOpenRustWorkspace={() => setIsRustCodebaseOpen(true)}
      />

      {/* 2. Desktop Menu Bar */}
      <MenuBar
        onOpenDirectPst={() => setIsDirectPstOpen(true)}
        onOpenCloudMigration={() => setIsCloudMigrationOpen(true)}
        onOpenFilter={() => setIsFilterOpen(true)}
        onOpenSplitter={() => setIsSplitterOpen(true)}
        onOpenDeltaSync={() => setIsDeltaSyncOpen(true)}
        onOpenForensics={() => setIsForensicsOpen(true)}
        onOpenLegacyFormats={() => setIsLegacyFormatsOpen(true)}
        onOpenRustCodebase={() => setIsRustCodebaseOpen(true)}
        onOpenExportReport={() => setIsExportReportOpen(true)}
        onOpenSecurityAudit={() => setIsSecurityAuditOpen(true)}
        onSelectAllFolders={handleSelectAllFolders}
        onClearFolderSelection={handleClearFolderSelection}
      />

      {/* 3. Action Ribbon Toolbar */}
      <RibbonBar
        onOpenDirectPst={() => setIsDirectPstOpen(true)}
        onOpenCloudMigration={() => setIsCloudMigrationOpen(true)}
        onOpenFilter={() => setIsFilterOpen(true)}
        onOpenSplitter={() => setIsSplitterOpen(true)}
        onOpenDeltaSync={() => setIsDeltaSyncOpen(true)}
        onOpenForensics={() => setIsForensicsOpen(true)}
        onOpenLegacyFormats={() => setIsLegacyFormatsOpen(true)}
        onOpenRustCodebase={() => setIsRustCodebaseOpen(true)}
        onOpenExportReport={() => setIsExportReportOpen(true)}
        onOpenSecurityAudit={() => setIsSecurityAuditOpen(true)}
        filterCriteria={filterCriteria}
        onResetFilters={handleResetFilters}
        totalFilteredCount={allFilteredMessages.length}
        totalOriginalCount={messages.length}
      />

      {/* 4. Main 3-Pane Explorer (Left: Folder Tree, Middle: Message List, Right: Message Preview) */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left Pane: Selective Folder Tree */}
        <FolderTreePane
          folders={folders}
          activeFolderId={activeFolderId}
          onSelectFolder={setActiveFolderId}
          onToggleFolderCheck={handleToggleFolderCheck}
          onSelectAllFolders={handleSelectAllFolders}
          onClearFolderSelection={handleClearFolderSelection}
        />

        {/* Middle Pane: Message List Table */}
        <MessageListPane
          messages={filteredFolderMessages}
          selectedMessageId={selectedMessageId}
          onSelectMessage={setSelectedMessageId}
          folderName={activeFolder ? activeFolder.name : 'Messages'}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          checkedMessageIds={checkedMessageIds}
          onToggleCheckMessage={handleToggleCheckMessage}
          onSelectAllVisible={handleSelectAllVisibleMessages}
          onClearCheckedMessages={handleClearCheckedMessages}
          onBulkDelete={handleBulkDeleteMessages}
        />

        {/* Right Pane: Multi-Tab Message Preview & Hex / Forensics */}
        <MessagePreviewPane
          message={selectedMessage}
          onToggleLegalHold={handleToggleLegalHold}
        />
      </div>

      {/* 5. Desktop Status Bar */}
      <StatusBar
        folders={folders}
        messages={messages}
        activeFolder={activeFolder}
        hasActiveFilters={allFilteredMessages.length < messages.length}
        onOpenRustCodebase={() => setIsRustCodebaseOpen(true)}
      />

      {/* Modals & Workbenches */}
      <DirectPstModal
        isOpen={isDirectPstOpen}
        onClose={() => setIsDirectPstOpen(false)}
        folders={folders}
        messages={messages}
      />

      <CloudMigrationModal
        isOpen={isCloudMigrationOpen}
        onClose={() => setIsCloudMigrationOpen(false)}
        folders={folders}
      />

      <FilterModal
        isOpen={isFilterOpen}
        onClose={() => setIsFilterOpen(false)}
        criteria={filterCriteria}
        onApplyCriteria={setFilterCriteria}
        allMessages={messages}
      />

      <PstSplitModal
        isOpen={isSplitterOpen}
        onClose={() => setIsSplitterOpen(false)}
        messages={messages}
      />

      <DeltaSyncModal
        isOpen={isDeltaSyncOpen}
        onClose={() => setIsDeltaSyncOpen(false)}
        checkpoint={checkpoint}
        onUpdateCheckpoint={setCheckpoint}
        currentMessages={messages}
      />

      <ForensicModal
        isOpen={isForensicsOpen}
        onClose={() => setIsForensicsOpen(false)}
        messages={messages}
        onUpdateMessages={setMessages}
      />

      <LegacyFormatsModal
        isOpen={isLegacyFormatsOpen}
        onClose={() => setIsLegacyFormatsOpen(false)}
        onImportSampleLegacy={handleImportSampleLegacy}
      />

      <RustCodebaseModal
        isOpen={isRustCodebaseOpen}
        onClose={() => setIsRustCodebaseOpen(false)}
      />

      <ExportReportModal
        isOpen={isExportReportOpen}
        onClose={() => setIsExportReportOpen(false)}
        messages={messages}
        folders={folders}
        checkpoint={checkpoint}
      />

      <SecurityAuditViewer
        isOpen={isSecurityAuditOpen}
        onClose={() => setIsSecurityAuditOpen(false)}
      />
    </div>
  );
}
