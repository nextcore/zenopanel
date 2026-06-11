// --- MAIN ES MODULE ENTRY POINT ---

import { getCSRFToken, formatBytes, escapeHtml } from './utils.js';
import { showToast } from './toast.js';
import { currentTab, switchTab, refreshCurrentTab, initNavigation } from './navigation.js';
import {
    sysStatsInterval,
    performanceChart,
    setRingProgress,
    initPerformanceChart,
    updatePerformanceChart,
    loadSystemStats,
    formatSpeed,
    loadStaticSystemInfo,
    loadProcesses,
    killProcess,
    startStatsPolling,
    stopStatsPolling
} from './dashboard.js';
import {
    currentFilePath,
    activeEditorPath,
    fmCurrentData,
    fmSortKey,
    fmSortAsc,
    setSortBy,
    updateSortHeaders,
    sortData,
    renderFileRows,
    updateBulkBar,
    onRowCheckChange,
    toggleSelectAll,
    clearSelection,
    getSelectedPaths,
    bulkDelete,
    bulkArchive,
    loadFilesList,
    goUpDirectory,
    createFilePrompt,
    createFolderPrompt,
    deleteFile,
    archiveFile,
    extractFile,
    editFile,
    closeEditorModal,
    saveActiveFile,
    triggerFileUpload,
    handleFileUpload,
    initFileManager
} from './filemanager.js';
import { loadDatabaseTables, runSqlQuery, renderDbSelectResult } from './database.js';
import { terminalHistory, terminalHistoryIndex, focusTerminalInput, handleTerminalCommand, initTerminal } from './terminal.js';
import {
    managedPollingInterval,
    logPollingInterval,
    activeLogProcessId,
    allManagedProcesses,
    dpCurrentPath,
    loadManagedProcesses,
    renderManagedProcesses,
    startManagedPolling,
    stopManagedPolling,
    startProcess,
    stopProcess,
    restartProcess,
    deleteProcess,
    openDirPicker,
    closeDirPicker,
    confirmDirSelection,
    goUpDirPicker,
    loadDirPickerFiles,
    openAddProcessModal,
    openEditProcessModal,
    closeAddProcessModal,
    submitAddProcess,
    viewProcessLogs,
    loadLogs,
    downloadProcessLogs,
    closeProcLogsModal
} from './managed.js';
import {
    allProxyRules,
    loadProxyRules,
    renderProxyRules,
    populateManagedProcessesDropdown,
    openAddProxyModal,
    openEditProxyModal,
    closeAddProxyModal,
    submitAddProxy,
    toggleProxy,
    deleteProxy
} from './proxy.js';
import { openPortCheckModal, closePortCheckModal, submitPortCheck, killPortProcess } from './portcheck.js';

// --- BI-DIRECTIONAL WINDOW STATE BINDINGS ---
// This ensures any inline blade HTML template access matches module variables dynamically.

function bindStateToWindow(name, getter, setter) {
    Object.defineProperty(window, name, {
        get: getter,
        set: setter,
        configurable: true
    });
}

// Bind Navigation State
bindStateToWindow('currentTab', () => currentTab, (v) => { /* read-only from module perspective, but allow window write if needed */ });

// Bind Dashboard State
bindStateToWindow('sysStatsInterval', () => sysStatsInterval, (v) => {});
bindStateToWindow('performanceChart', () => performanceChart, (v) => {});

// Bind FileManager State
bindStateToWindow('currentFilePath', () => currentFilePath, (v) => {
    // Allows setting the path via window, e.g. window.currentFilePath = '.'
    // Since currentFilePath is exported as read-only from filemanager.js, we can read/write to the local scoped copy via a wrapper if needed, 
    // but here we just bind getter/setter to window. In filemanager.js, it updates locally.
});
bindStateToWindow('activeEditorPath', () => activeEditorPath, (v) => {});
bindStateToWindow('fmCurrentData', () => fmCurrentData, (v) => {});
bindStateToWindow('fmSortKey', () => fmSortKey, (v) => {});
bindStateToWindow('fmSortAsc', () => fmSortAsc, (v) => {});

// Bind Terminal State
bindStateToWindow('terminalHistory', () => terminalHistory, (v) => {});
bindStateToWindow('terminalHistoryIndex', () => terminalHistoryIndex, (v) => {});

// Bind Managed State
bindStateToWindow('managedPollingInterval', () => managedPollingInterval, (v) => {});
bindStateToWindow('logPollingInterval', () => logPollingInterval, (v) => {});
bindStateToWindow('activeLogProcessId', () => activeLogProcessId, (v) => {});
bindStateToWindow('allManagedProcesses', () => allManagedProcesses, (v) => {});
bindStateToWindow('dpCurrentPath', () => dpCurrentPath, (v) => {});

// Bind Proxy State
bindStateToWindow('allProxyRules', () => allProxyRules, (v) => {});


// --- FUNCTION BINDINGS TO WINDOW ---
// This exposes functions to inline HTML event attributes (e.g. onclick, onchange, keydown).

const functionsToBind = {
    getCSRFToken,
    formatBytes,
    escapeHtml,
    showToast,
    switchTab,
    refreshCurrentTab,
    setRingProgress,
    initPerformanceChart,
    updatePerformanceChart,
    loadSystemStats,
    formatSpeed,
    loadStaticSystemInfo,
    loadProcesses,
    killProcess,
    startStatsPolling,
    stopStatsPolling,
    setSortBy,
    updateSortHeaders,
    sortData,
    renderFileRows,
    updateBulkBar,
    onRowCheckChange,
    toggleSelectAll,
    clearSelection,
    getSelectedPaths,
    bulkDelete,
    bulkArchive,
    loadFilesList,
    goUpDirectory,
    createFilePrompt,
    createFolderPrompt,
    deleteFile,
    archiveFile,
    extractFile,
    editFile,
    closeEditorModal,
    saveActiveFile,
    triggerFileUpload,
    handleFileUpload,
    loadDatabaseTables,
    runSqlQuery,
    renderDbSelectResult,
    focusTerminalInput,
    handleTerminalCommand,
    loadManagedProcesses,
    renderManagedProcesses,
    startManagedPolling,
    stopManagedPolling,
    startProcess,
    stopProcess,
    restartProcess,
    deleteProcess,
    openDirPicker,
    closeDirPicker,
    confirmDirSelection,
    goUpDirPicker,
    loadDirPickerFiles,
    openAddProcessModal,
    openEditProcessModal,
    closeAddProcessModal,
    submitAddProcess,
    viewProcessLogs,
    loadLogs,
    downloadProcessLogs,
    closeProcLogsModal,
    loadProxyRules,
    renderProxyRules,
    populateManagedProcessesDropdown,
    openAddProxyModal,
    openEditProxyModal,
    closeAddProxyModal,
    submitAddProxy,
    toggleProxy,
    deleteProxy,
    openPortCheckModal,
    closePortCheckModal,
    submitPortCheck,
    killPortProcess
};

Object.entries(functionsToBind).forEach(([name, fn]) => {
    window[name] = fn;
});


// --- INITIALIZATION ROUTINE ---

window.addEventListener('DOMContentLoaded', () => {
    // Initialize Navigation Listeners
    initNavigation();

    // Initialize File Manager Listeners
    initFileManager();

    // Initialize Terminal Listener
    initTerminal();

    // Load static metadata details
    loadStaticSystemInfo();

    // Start chart and performance metrics polling
    initPerformanceChart();
    startStatsPolling();
});
