import { showToast } from "./toast.js";
import {
  loadSystemStats,
  startStatsPolling,
  stopStatsPolling,
} from "./dashboard.js";
import { loadFilesList, initFileManager } from "./filemanager.js";
import { initDatabaseTab } from "./database.js";
import { focusTerminalInput, initTerminal } from "./terminal.js";
import {
  loadManagedProcesses,
  startManagedPolling,
  stopManagedPolling,
} from "./managed.js";
import { loadProxyRules } from "./proxy.js";
import {
  loadContainers,
  startContainerPolling,
  stopContainerPolling,
} from "./containers.js";
import { loadUsers } from "./users.js";
import { loadSettings, loadSecuritySettings } from "./settings.js";

// Tab Navigation state
export let currentTab = "dashboard";

export function switchTab(tab) {
  // Update nav active button class
  document
    .querySelectorAll(".nav-item")
    .forEach((el) => el.classList.remove("active"));
  const activeBtn = document.querySelector(`.nav-item[data-tab="${tab}"]`);
  if (activeBtn) activeBtn.classList.add("active");

  currentTab = tab;

  // Let HTMX load the content programmatically if it's not already loading it
  if (typeof htmx !== "undefined") {
    htmx.ajax("GET", `/tab/${tab}`, "#viewport-container");
  }
}

export function runTabInit(tab) {
  const pageTitle = document.getElementById("page-title");
  if (pageTitle) {
    pageTitle.innerText = tab.charAt(0).toUpperCase() + tab.slice(1);
  }

  // Manage all pollers (stop pollers of other tabs, start pollers of active tab)
  if (tab === "dashboard") {
    loadSystemStats();
    startStatsPolling();
  } else {
    stopStatsPolling();
  }

  if (tab === "files") {
    initFileManager();
    const path = window.currentFilePath || ".";
    loadFilesList(path);
  }

  if (tab === "database") {
    initDatabaseTab();
  }

  if (tab === "terminal") {
    initTerminal();
    setTimeout(focusTerminalInput, 50);
  }

  if (tab === "managed") {
    loadManagedProcesses();
    startManagedPolling();
  } else {
    stopManagedPolling();
  }

  if (tab === "proxy") {
    loadProxyRules();
  }

  if (tab === "containers") {
    loadContainers();
    startContainerPolling();
  } else {
    stopContainerPolling();
  }

  if (tab === "users") {
    loadUsers();
  }

  if (tab === "settings") {
    loadSettings();
  }

  if (tab === "security") {
    loadSecuritySettings();
  }
}

// Global refresh trigger
export function refreshCurrentTab() {
  if (typeof htmx !== "undefined") {
    // Re-request active tab content via HTMX
    htmx.ajax("GET", `/tab/${currentTab}`, "#viewport-container");
  } else {
    runTabInit(currentTab);
  }
  showToast("success", "Refreshed tab data successfully");
}

// Setup navigation event listeners
export function initNavigation() {
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", () => {
      document
        .querySelectorAll(".nav-item")
        .forEach((el) => el.classList.remove("active"));
      item.classList.add("active");
      
      const tab = item.getAttribute("data-tab");
      currentTab = tab;
    });
  });

  // Listen to HTMX afterSwap events to run tab initializers
  document.body.addEventListener("htmx:afterSwap", (evt) => {
    if (evt.detail.target.id === "viewport-container") {
      runTabInit(currentTab);
    }
  });
}
