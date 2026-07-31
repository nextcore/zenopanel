import { showToast } from "./toast.js";
import {
  loadSystemStats,
  startStatsPolling,
  stopStatsPolling,
  initPerformanceChart,
  initTrafficChart,
  loadStaticSystemInfo,
} from "./dashboard.js";
import { loadFilesList, initFileManager } from "./filemanager.js";
import { initDatabaseTab } from "./database.js";
import { focusTerminalInput, initTerminal, closeTerminal } from "./terminal.js";
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
  updateContainerSparklines,
} from "./containers.js";
import { loadUsers } from "./users.js";
import { loadSettings, loadSecuritySettings, loadFirewallRules } from "./settings.js";
import { loadCronJobs } from "./cron.js";
import { loadZenoMachines, updateMachineSparklines } from "./machines.js";

// Tab Navigation state
export let currentTab = "dashboard";
let metricsPollingInterval = null;

function startMetricsPolling() {
  if (metricsPollingInterval) return;
  metricsPollingInterval = setInterval(() => {
    fetch('/api/metrics')
      .then(res => res.json())
      .then(res => {
        if (!res.success) return;
        if (res.containers && currentTab === 'containers') {
          updateContainerSparklines(res.containers);
        }
        if (res.machines && currentTab === 'machines') {
          updateMachineSparklines(res.machines);
        }
      })
      .catch(() => {});
  }, 3000);
}

function stopMetricsPolling() {
  if (metricsPollingInterval) {
    clearInterval(metricsPollingInterval);
    metricsPollingInterval = null;
  }
}

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
    htmx.ajax("GET", `/tab/${tab}`, {
      target: "#viewport-container",
      swap: "innerHTML scroll:none show:none",
    });
  }
}

export function runTabInit(tab) {
  const pageTitle = document.getElementById("page-title");
  if (pageTitle) {
    pageTitle.innerText = tab.charAt(0).toUpperCase() + tab.slice(1);
  }

  // Force active viewport to scroll to top (prevents layout cut-off on swap/refresh)
  const activeViewport = document.querySelector(".viewport.active");
  if (activeViewport) {
    activeViewport.scrollTop = 0;
    // Reset scroll at multiple intervals to counteract layout shifts during charts and table rendering
    [50, 150, 300, 600].forEach(delay => {
      setTimeout(() => {
        activeViewport.scrollTop = 0;
      }, delay);
    });
  }

  // Manage all pollers (stop pollers of other tabs, start pollers of active tab)
  if (tab === "dashboard") {
    initPerformanceChart();
    initTrafficChart();
    loadSystemStats();
    startStatsPolling();
    loadStaticSystemInfo();
  } else {
    stopStatsPolling();
  }

  if (tab === "files") {
    initFileManager();
    const path = window.currentFilePath || "/var/www";
    loadFilesList(path);
  }

  if (tab === "database") {
    initDatabaseTab();
  }

  if (tab === "terminal") {
    initTerminal();
    setTimeout(focusTerminalInput, 50);
  } else {
    closeTerminal();
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
    startMetricsPolling();
  } else {
    stopContainerPolling();
  }

  if (tab === "users") {
    loadUsers();
  }

  if (tab === "cron") {
    loadCronJobs();
  }

  if (tab === "settings") {
    loadSettings();
  }

  if (tab === "security" || tab === "waf") {
    loadSecuritySettings();
  }

  if (tab === "firewall") {
    loadFirewallRules();
  }

  if (tab === "machines") {
    loadZenoMachines();
    startMetricsPolling();
  }

  if (tab !== "containers" && tab !== "machines") {
    stopMetricsPolling();
  }

  // Expand or collapse Security sub-menu based on active tab
  const submenu = document.getElementById("security-submenu");
  const arrow = document.getElementById("security-arrow");
  if (tab === "firewall" || tab === "waf" || tab === "security") {
    if (submenu) submenu.style.display = "flex";
    if (arrow) arrow.style.transform = "rotate(-180deg)";
  } else {
    if (submenu) submenu.style.display = "none";
    if (arrow) arrow.style.transform = "rotate(0deg)";
  }
}

// Global refresh trigger
export function refreshCurrentTab() {
  if (typeof htmx !== "undefined") {
    // Re-request active tab content via HTMX
    htmx.ajax("GET", `/tab/${currentTab}`, {
      target: "#viewport-container",
      swap: "innerHTML scroll:none show:none",
    });
  } else {
    runTabInit(currentTab);
  }
  showToast("success", "Refreshed tab data successfully");
}

// Toggle mobile sidebar view
export function toggleMobileSidebar() {
  const aside = document.querySelector("aside");
  const overlay = document.querySelector(".sidebar-overlay");
  if (aside && overlay) {
    aside.classList.toggle("active");
    overlay.classList.toggle("active");
  }
}

// Toggle desktop sidebar slide collapse
export function toggleDesktopSidebar() {
  const aside = document.getElementById("main-sidebar") || document.querySelector("aside");
  const icon = document.getElementById("sidebar-collapse-icon");
  if (aside) {
    aside.classList.toggle("collapsed");
    const isCollapsed = aside.classList.contains("collapsed");
    localStorage.setItem("zenopanel_sidebar_collapsed", isCollapsed ? "true" : "false");
    if (icon) {
      icon.className = isCollapsed ? "fa-solid fa-angles-right" : "fa-solid fa-angles-left";
    }
  }
}

// Universal sidebar toggle for header button
export function toggleSidebar() {
  if (window.innerWidth <= 768) {
    toggleMobileSidebar();
  } else {
    toggleDesktopSidebar();
  }
}

// Restore saved sidebar collapse preference on boot
export function initSidebarState() {
  const savedState = localStorage.getItem("zenopanel_sidebar_collapsed");
  if (savedState === "true" && window.innerWidth > 768) {
    const aside = document.getElementById("main-sidebar") || document.querySelector("aside");
    const icon = document.getElementById("sidebar-collapse-icon");
    if (aside) aside.classList.add("collapsed");
    if (icon) icon.className = "fa-solid fa-angles-right";
  }
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

      // Close mobile sidebar if active on navigation
      const aside = document.querySelector("aside");
      const overlay = document.querySelector(".sidebar-overlay");
      if (aside && overlay) {
        aside.classList.remove("active");
        overlay.classList.remove("active");
      }
    });
  });

  // Listen to HTMX afterSwap events to run tab initializers
  document.body.addEventListener("htmx:afterSwap", (evt) => {
    if (evt.detail.target.id === "viewport-container") {
      runTabInit(currentTab);
    }
  });
}

export function toggleSecuritySubmenu(event) {
  if (event) {
    event.stopPropagation();
  }
  const submenu = document.getElementById("security-submenu");
  const arrow = document.getElementById("security-arrow");
  if (submenu && arrow) {
    if (submenu.style.display === "none" || submenu.style.display === "") {
      submenu.style.display = "flex";
      arrow.style.transform = "rotate(-180deg)";
    } else {
      submenu.style.display = "none";
      arrow.style.transform = "rotate(0deg)";
    }
  }
}

window.switchTab = switchTab;
window.toggleSecuritySubmenu = toggleSecuritySubmenu;
