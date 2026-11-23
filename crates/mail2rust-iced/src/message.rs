use crate::state::{ActiveModal, ActiveTab, ExportFormat, ExportScope, SortColumn};
use mail2rust_pst::PstEncryption;

#[derive(Debug, Clone)]
pub enum Message {
    // Layout & Workbench Navigation
    SelectFolder(String),
    ToggleFolderChecked(usize),
    SelectAllFolders,
    SelectNoFolders,
    SelectEmail(String),
    ToggleMessageChecked(String),
    SelectAllVisible,
    ClearSelection,
    InvertSelection,
    ToggleSort(SortColumn),
    SearchChanged(String),
    ClearSearch,
    SetActiveTab(ActiveTab),
    ToggleTreePane,
    TogglePreviewPane,
    ToggleDarkMode,
    RefreshWorkspace,
    LoadSampleData,
    ClearData,
    CopyText(String),

    // Direct Native File Pickers
    QuickOpenFile,
    QuickOpenFolder,

    // Modal Lifecycle
    OpenModal(ActiveModal),
    CloseModal,

    // Open Mailbox Modal
    OpenMailboxPathChanged(String),
    BrowseOpenMailboxFile,
    BrowseOpenMailboxFolder,
    ExecuteOpenMailbox,

    // IMAP Connect Modal
    ApplyImapPreset { host: String, port: String, insecure: bool },
    ImapHostChanged(String),
    ImapPortChanged(String),
    ImapAuthMethodChanged(String),
    ImapUserChanged(String),
    ImapPassChanged(String),
    ImapTokenChanged(String),
    ImapTlsToggled(bool),
    ImapInsecureToggled(bool),
    ImapFoldersChanged(String),
    ImapMaxChanged(String),
    ExecuteImapConnect,

    // IMAP Sync Modal
    ImapSyncSrcHostChanged(String),
    ImapSyncSrcPortChanged(String),
    ImapSyncSrcUserChanged(String),
    ImapSyncSrcPassChanged(String),
    ImapSyncSrcAuthMethodChanged(String),
    ImapSyncSrcTokenChanged(String),
    ImapSyncSrcTlsToggled(bool),
    ImapSyncDstHostChanged(String),
    ImapSyncDstPortChanged(String),
    ImapSyncDstUserChanged(String),
    ImapSyncDstPassChanged(String),
    ImapSyncDstAuthMethodChanged(String),
    ImapSyncDstTokenChanged(String),
    ImapSyncDstTlsToggled(bool),
    ImapSyncInsecureToggled(bool),
    ImapSyncFoldersChanged(String),
    ImapSyncPrefixChanged(String),
    ImapSyncFlagsToggled(bool),
    ImapSyncDatesToggled(bool),
    ImapSyncDryRunToggled(bool),
    ExecuteImapSync,

    // Merge Mailboxes Modal
    MergeInputChanged(String),
    MergeAddInputPath,
    MergeBrowseAddFile,
    MergeBrowseAddFolder,
    MergeAddSamples,
    MergeClearSources,
    MergeRemoveSource(usize),
    MergeFolderStrategyChanged(String),
    MergeDedupStrategyChanged(String),
    MergeFormatChanged(ExportFormat),
    MergeSortToggled(bool),
    MergeDestChanged(String),
    MergeBrowseDest,
    ExecuteMergeToWorkbench,
    ExecuteMergeToFile,

    // Export / Direct PST Modal
    ExportScopeChanged(ExportScope),
    ExportFormatChanged(ExportFormat),
    ExportDestChanged(String),
    ExportBrowseDest,
    ExportRootChanged(String),
    ExportMaxGbChanged(String),
    ExportClusterToggled(bool),
    ExportEncryptionChanged(PstEncryption),
    ExportPasswordChanged(String),
    ExecuteExport,

    // Auto-Split Modal
    AutoSplitSelectGb(usize),
    ExecuteAutoSplit,

    // Filter Criteria Modal
    FilterSinceChanged(String),
    FilterUntilChanged(String),
    FilterIncludeGlobChanged(String),
    FilterExcludeGlobChanged(String),
    FilterIncludeDeletedToggled(bool),
    ResetFilters,

    // Forensics Modal
    CaseNumberChanged(String),
    ExaminerChanged(String),
    BatesPrefixChanged(String),
    BatesStartChanged(String),
    ManifestCsvChanged(String),
    BrowseManifestCsv,
    ExecuteForensicsStamping,

    // Validate PST Modal
    ValidatePstPathChanged(String),
    BrowseValidatePstFile,
    ValidatePstPasswordChanged(String),
    ExecuteValidatePst,
}
