// --- MAIN ES MODULE ENTRY POINT ---

import {
  getCSRFToken,
  formatBytes,
  escapeHtml,
  getCookieValue,
} from "./utils.js";
import { showToast } from "./toast.js";
import {
  currentTab,
  switchTab,
  refreshCurrentTab,
  initNavigation,
} from "./navigation.js";
import {
  sysStatsInterval,
  performanceChart,
  setRingProgress,
  initPerformanceChart,
  initTrafficChart,
  loadTrafficStats,
  updatePerformanceChart,
  loadSystemStats,
  formatSpeed,
  loadStaticSystemInfo,
  loadProcesses,
  killProcess,
  startStatsPolling,
  stopStatsPolling,
} from "./dashboard.js";
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
  initFileManager,
  changePermissionsPrompt,
  closePermissionsModal,
  submitChangePermissions,
  updateOctalFromCheckboxes,
  updateCheckboxesFromOctal,
} from "./filemanager.js";
import {
  loadDatabaseTables,
  runSqlQuery,
  renderDbSelectResult,
} from "./database.js";
import {
  terminalHistory,
  terminalHistoryIndex,
  focusTerminalInput,
  handleTerminalCommand,
  initTerminal,
} from "./terminal.js";
import {
  managedPollingInterval,
  logPollingInterval,
  activeLogProcessId,
  managedState,
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
  closeProcLogsModal,
  toggleProcessDropdown,
} from "./managed.js";
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
  deleteProxy,
} from "./proxy.js";
import {
  containerState,
  loadContainers,
  renderContainers,
  startContainerPolling,
  stopContainerPolling,
  startContainer,
  stopContainer,
  deleteContainer,
  inspectContainer,
  openAddContainerModal,
  closeAddContainerModal,
  addEnvRow,
  submitAddContainer,
  openContainerImagesModal,
  closeContainerImagesModal,
  pullContainerImage,
  toggleContainerDropdown,
  addPortRow,
  addVolumeRow,
  viewContainerLogs,
  closeContainerLogsModal,
  deleteImage,
  browseContainerFiles,
  loadCachedImagesForCreate,
  selectCachedImage,
  switchContainerSubTab,
  openContainerProxyModal,
  closeContainerProxyModal,
  submitContainerProxy,
} from "./containers.js";
import {
  openPortCheckModal,
  closePortCheckModal,
  submitPortCheck,
  killPortProcess,
} from "./portcheck.js";
import { composeUp, composeDown, composePs } from "./compose.js";
import {
  allUsers,
  loadUsers,
  openAddUserModal,
  openEditUserModal,
  closeAddUserModal,
  submitAddUser,
  deleteUser,
} from "./users.js";
import {
  loadSettings,
  submitSaveSettings,
  loadServiceStatus,
  installService,
  uninstallService,
  copyInstallCmd,
  loadSecuritySettings,
  submitSaveSecurity,
  toggleRateLimitFields,
} from "./settings.js";

// --- BI-DIRECTIONAL WINDOW STATE BINDINGS ---
// This ensures any inline blade HTML template access matches module variables dynamically.

function bindStateToWindow(name, getter, setter) {
  Object.defineProperty(window, name, {
    get: getter,
    set: setter,
    configurable: true,
  });
}

// Bind Navigation State
bindStateToWindow(
  "currentTab",
  () => currentTab,
  (v) => {
    /* read-only from module perspective, but allow window write if needed */
  },
);

// Bind Dashboard State
bindStateToWindow(
  "sysStatsInterval",
  () => sysStatsInterval,
  (v) => {},
);
bindStateToWindow(
  "performanceChart",
  () => performanceChart,
  (v) => {},
);

// Bind FileManager State
bindStateToWindow(
  "currentFilePath",
  () => currentFilePath,
  (v) => {
    // Allows setting the path via window, e.g. window.currentFilePath = '.'
    // Since currentFilePath is exported as read-only from filemanager.js, we can read/write to the local scoped copy via a wrapper if needed,
    // but here we just bind getter/setter to window. In filemanager.js, it updates locally.
  },
);
bindStateToWindow(
  "activeEditorPath",
  () => activeEditorPath,
  (v) => {},
);
bindStateToWindow(
  "fmCurrentData",
  () => fmCurrentData,
  (v) => {},
);
bindStateToWindow(
  "fmSortKey",
  () => fmSortKey,
  (v) => {},
);
bindStateToWindow(
  "fmSortAsc",
  () => fmSortAsc,
  (v) => {},
);

// Bind Terminal State
bindStateToWindow(
  "terminalHistory",
  () => terminalHistory,
  (v) => {},
);
bindStateToWindow(
  "terminalHistoryIndex",
  () => terminalHistoryIndex,
  (v) => {},
);

// Bind Managed State
bindStateToWindow(
  "managedPollingInterval",
  () => managedPollingInterval,
  (v) => {},
);
bindStateToWindow(
  "logPollingInterval",
  () => logPollingInterval,
  (v) => {},
);
bindStateToWindow(
  "activeLogProcessId",
  () => activeLogProcessId,
  (v) => {},
);
bindStateToWindow(
  "allManagedProcesses",
  () => managedState.allManagedProcesses,
  (v) => {},
);
bindStateToWindow(
  "dpCurrentPath",
  () => dpCurrentPath,
  (v) => {},
);

// Bind Proxy State
bindStateToWindow(
  "allProxyRules",
  () => allProxyRules,
  (v) => {},
);

// Bind Users State
bindStateToWindow(
  "allUsers",
  () => allUsers,
  (v) => {},
);

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
  initTrafficChart,
  loadTrafficStats,
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
  changePermissionsPrompt,
  closePermissionsModal,
  submitChangePermissions,
  updateOctalFromCheckboxes,
  updateCheckboxesFromOctal,
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
  toggleProcessDropdown,
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
  killPortProcess,
  loadUsers,
  openAddUserModal,
  openEditUserModal,
  closeAddUserModal,
  submitAddUser,
  deleteUser,
  loadSettings,
  submitSaveSettings,
  loadServiceStatus,
  installService,
  uninstallService,
  copyInstallCmd,
  loadSecuritySettings,
  submitSaveSecurity,
  toggleRateLimitFields,
  loadContainers,
  renderContainers,
  startContainerPolling,
  stopContainerPolling,
  startContainer,
  stopContainer,
  deleteContainer,
  inspectContainer,
  openAddContainerModal,
  closeAddContainerModal,
  addEnvRow,
  submitAddContainer,
  openContainerImagesModal,
  closeContainerImagesModal,
  pullContainerImage,
  toggleContainerDropdown,
  addPortRow,
  addVolumeRow,
  viewContainerLogs,
  closeContainerLogsModal,
  deleteImage,
  browseContainerFiles,
  loadCachedImagesForCreate,
  selectCachedImage,
  switchContainerSubTab,
  composeUp,
  composeDown,
  composePs,
  openContainerProxyModal,
  closeContainerProxyModal,
  submitContainerProxy,
};

Object.entries(functionsToBind).forEach(([name, fn]) => {
  window[name] = fn;
});

// Also add compose functions to window
window.composeUp = composeUp;
window.composeDown = composeDown;
window.composePs = composePs;

// --- INITIALIZATION ROUTINE ---

window.addEventListener("DOMContentLoaded", () => {
  // Apply role-based client-side gating
  const userRole = getCookieValue("zeno_role") || "viewer";
  window.userRole = userRole;

  if (userRole === "admin") {
    const navUsers = document.getElementById("nav-users");
    if (navUsers) navUsers.style.display = "flex";
    const navSecurity = document.getElementById("nav-security");
    if (navSecurity) navSecurity.style.display = "flex";
    const navSettings = document.getElementById("nav-settings");
    if (navSettings) navSettings.style.display = "flex";
  } else {
    const dbTab = document.querySelector('.nav-item[data-tab="database"]');
    if (dbTab) dbTab.style.display = "none";

    const termTab = document.querySelector('.nav-item[data-tab="terminal"]');
    if (termTab) termTab.style.display = "none";

    const usersTab = document.querySelector('.nav-item[data-tab="users"]');
    if (usersTab) usersTab.style.display = "none";

    const securityTab = document.querySelector(
      '.nav-item[data-tab="security"]',
    );
    if (securityTab) securityTab.style.display = "none";
  }

  if (userRole === "viewer") {
    const style = document.createElement("style");
    style.innerHTML = `
            .btn-primary-action,
            .btn-danger-action,
            .btn-action[onclick*="Delete"],
            .btn-action[onclick*="Edit"],
            .btn-action[onclick*="Upload"],
            .btn-action[onclick*="Create"],
            .btn-action[onclick*="openAdd"],
            .btn-action[onclick*="openEdit"],
            button[onclick*="openAdd"],
            button[onclick*="openEdit"],
            button[onclick*="edit"],
            #tab-files button[onclick*="delete"],
            #tab-files button[onclick*="changePermissionsPrompt"],
            #tab-managed button[onclick*="startProcess"],
            #tab-managed button[onclick*="stopProcess"],
            #tab-managed button[onclick*="restartProcess"],
            #tab-managed button[onclick*="openEditProcessModal"],
            #tab-managed button[onclick*="viewProcessLogs"],
            #tab-managed .action-dropdown-menu hr,
            #tab-users .data-table th:last-child,
            #tab-users .data-table td:last-child,
            #tab-proxy .data-table th:last-child,
            #tab-proxy .data-table td:last-child,
            .file-actions-bar,
            #upload-trigger,
            .upload-btn {
                display: none !important;
            }
        `;
    document.head.appendChild(style);
  }

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
  initTrafficChart();
  startStatsPolling();

  // Close all action dropdown menus when clicking outside
  document.addEventListener("click", () => {
    const allMenus = document.querySelectorAll(".action-dropdown-menu");
    const hadOpen =
      allMenus.length > 0 &&
      [...allMenus].some((m) => m.classList.contains("show"));
    allMenus.forEach((menu) => {
      menu.classList.remove("show");
      menu.style.position = "";
      menu.style.top = "";
      menu.style.left = "";
      menu.style.right = "";
      menu.style.bottom = "";
      if (menu.parentElement) {
        menu.parentElement.classList.remove("open-up");
      }
    });
    if (hadOpen) {
      // Resume container polling if it was paused by an open dropdown
      try {
        startContainerPolling();
      } catch (e) {}
    }
  });

  // Close dropdowns on scroll (fixed-position menus don't follow scroll)
  document.addEventListener(
    "scroll",
    () => {
      const allMenus = document.querySelectorAll(".action-dropdown-menu.show");
      const hadOpen = allMenus.length > 0;
      allMenus.forEach((menu) => {
        menu.classList.remove("show");
        menu.style.position = "";
        menu.style.top = "";
        menu.style.left = "";
        menu.style.right = "";
        menu.style.bottom = "";
        if (menu.parentElement) {
          menu.parentElement.classList.remove("open-up");
        }
      });
      if (hadOpen) {
        try {
          startContainerPolling();
        } catch (e) {}
      }
    },
    true,
  ); // capture phase to catch all scroll events
});
