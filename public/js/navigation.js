import { showToast } from "./toast.js";
import {
  loadSystemStats,
  startStatsPolling,
  stopStatsPolling,
} from "./dashboard.js";
import { loadFilesList } from "./filemanager.js";
import { initDatabaseTab } from "./database.js";
import { focusTerminalInput } from "./terminal.js";
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
  document
    .querySelectorAll(".nav-item")
    .forEach((el) => el.classList.remove("active"));
  document
    .querySelectorAll(".viewport")
    .forEach((el) => el.classList.remove("active"));

  const activeBtn = document.querySelector(`.nav-item[data-tab="${tab}"]`);
  if (activeBtn) activeBtn.classList.add("active");

  const activeViewport = document.getElementById(`tab-${tab}`);
  if (activeViewport) activeViewport.classList.add("active");

  currentTab = tab;

  const pageTitle = document.getElementById("page-title");
  if (pageTitle) {
    pageTitle.innerText = tab.charAt(0).toUpperCase() + tab.slice(1);
  }

  // Tab initialization routines
  if (tab === "dashboard") {
    loadSystemStats();
    startStatsPolling();
  } else {
    stopStatsPolling();
  }

  if (tab === "files") {
    // Get the currentFilePath from window or import dynamically if needed
    // Since we bind state variables to window for backward compatibility,
    // window.currentFilePath is a safe fallback.
    const path = window.currentFilePath || ".";
    loadFilesList(path);
  }

  if (tab === "database") {
    initDatabaseTab();
  }

  if (tab === "terminal") {
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
  switchTab(currentTab);
  showToast("success", "Refreshed tab data successfully");
}

// Setup navigation event listeners
export function initNavigation() {
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", () => {
      const tab = item.getAttribute("data-tab");
      switchTab(tab);
    });
  });
}
