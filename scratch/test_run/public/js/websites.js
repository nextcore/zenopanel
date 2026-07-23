import { getCSRFToken, escapeHtml } from "./utils.js";
import { showToast } from "./toast.js";
import { switchTab } from "./navigation.js";

// Websites Management Module

export function initWebsitesTab() {
  loadWebsites();
}

export function loadWebsites() {
  const container = document.getElementById("websites-list-container");
  if (!container) return;

  container.innerHTML = `
    <tr>
      <td colspan="6" style="text-align:center; padding: 30px; color: var(--text-muted);">
        <i class="fa-solid fa-spinner fa-spin" style="margin-right: 8px;"></i> Loading websites...
      </td>
    </tr>
  `;

  // Fetch both containers and proxy rules to reconstruct the "Websites" concept
  Promise.all([
    fetch("/api/containers/list").then((res) => res.json()).catch(() => ({ data: [] })),
    fetch("/api/proxy/list").then((res) => res.json()).catch(() => ({ data: [] })),
  ])
    .then(([containersRes, proxyRes]) => {
      const containers = Array.isArray(containersRes.data) ? containersRes.data : [];
      const proxies = Array.isArray(proxyRes.data) ? proxyRes.data : [];

      // Filter proxies representing websites (proxies with a managed_process_id starting with 'web_')
      const websiteProxies = proxies.filter(
        (p) => p.managed_process_id && p.managed_process_id.startsWith("web_")
      );

      if (websiteProxies.length === 0) {
        container.innerHTML = `
          <tr>
            <td colspan="6" style="text-align:center; padding: 40px; color: var(--text-muted);">
              <i class="fa-solid fa-globe" style="font-size: 2rem; display:block; margin-bottom:12px; opacity:0.3;"></i>
              No websites created yet. Click "Add Website" to get started.
            </td>
          </tr>
        `;
        return;
      }

      container.innerHTML = "";
      websiteProxies.forEach((proxy) => {
        let status = "stopped";
        let appType = "Unknown";
        let hostPort = "80";

        if (proxy.rule_type === "static") {
          status = "running";
          appType = "Static HTML";
          hostPort = "Static";
        } else {
          // Find matching container
          const cont = containers.find((c) => c.id === proxy.managed_process_id);
          status = cont ? cont.status : "stopped";
          appType = cont ? getAppTypeFromImage(cont.image) : "Unknown";
          
          // Parse host port from target URL (e.g. http://127.0.0.1:9001)
          try {
            const url = new URL(proxy.target);
            hostPort = url.port || "80";
          } catch (e) {}
        }

        const tr = document.createElement("tr");
        
        const badgeClass = status === "running" ? "badge-running" : "badge-stopped";
        const statusText = status.charAt(0).toUpperCase() + status.slice(1);
        
        let badgeHtml = `<span class="badge ${badgeClass}">${statusText}</span>`;
        if (status === "oom_killed") {
          badgeHtml = `<span class="badge" style="background:rgba(239,68,68,0.1); color:var(--danger); border:1px solid rgba(239,68,68,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem; display:inline-flex; align-items:center; gap:6px;"><i class="fa-solid fa-triangle-exclamation"></i> OOM Killed</span>`;
        }

        const toggleBtnHtml = proxy.rule_type === "static"
          ? `<button class="btn-action" disabled style="padding: 6px 10px; font-size: 0.78rem; opacity: 0.5; cursor: not-allowed;">
               <i class="fa-solid fa-cloud" style="color: var(--accent-primary);"></i>
               <span>Hosted</span>
             </button>`
          : `<button class="btn-action" onclick="toggleWebsite('${proxy.domain}', '${proxy.managed_process_id}', '${status}')" style="padding: 6px 10px; font-size: 0.78rem;">
               <i class="fa-solid ${status === "running" ? "fa-pause" : "fa-play"}" style="color: ${status === "running" ? "var(--warning)" : "var(--success)"};"></i>
               <span>${status === "running" ? "Stop" : "Start"}</span>
             </button>`;

        tr.innerHTML = `
          <td>
            <div style="font-weight: 600; color: #fff; display: flex; align-items: center; gap: 8px;">
              <i class="fa-solid fa-earth-americas" style="color: var(--accent-primary);"></i>
              <a href="http://${proxy.domain}" target="_blank" style="color: #fff; text-decoration: none; border-bottom: 1px dashed rgba(255,255,255,0.25);">${proxy.domain}</a>
            </div>
          </td>
          <td>
            <span style="font-family: var(--font-code); font-size: 0.8rem; background: rgba(255,255,255,0.05); padding: 2px 6px; border-radius: 4px; color: var(--text-main);">
              ${appType}
            </span>
          </td>
          <td style="font-family: var(--font-code); color: var(--text-muted);">${hostPort}</td>
          <td>
            ${badgeHtml}
          </td>
          <td>
            <div style="display:flex; gap: 6px;">
              ${toggleBtnHtml}
              <button class="btn-action" onclick="openWebSettingsModal('${proxy.domain}', '${proxy.managed_process_id}', '${proxy.id}')" style="padding: 6px 10px; font-size: 0.78rem;">
                <i class="fa-solid fa-gear" style="color: var(--accent-primary);"></i>
                <span>Settings</span>
              </button>
              <button class="btn-action" onclick="browseWebsiteFiles('${proxy.domain}', '${proxy.rule_type}')" style="padding: 6px 10px; font-size: 0.78rem;">
                <i class="fa-solid fa-folder-open" style="color: var(--accent-secondary);"></i>
                <span>Files</span>
              </button>
              <button class="btn-action" onclick="deleteWebsite('${proxy.domain}', '${proxy.managed_process_id}')" style="padding: 6px 10px; font-size: 0.78rem; background: rgba(239, 68, 68, 0.05); border-color: rgba(239, 68, 68, 0.15); color: #fca5a5;">
                <i class="fa-solid fa-trash-can" style="color: var(--danger);"></i>
                <span>Delete</span>
              </button>
            </div>
          </td>
        `;
        container.appendChild(tr);
      });
    })
    .catch((err) => {
      console.error(err);
      container.innerHTML = `
        <tr>
          <td colspan="6" style="text-align:center; padding: 30px; color: var(--danger);">
            <i class="fa-solid fa-triangle-exclamation" style="margin-right: 8px;"></i> Failed to load websites list.
          </td>
        </tr>
      `;
    });
}

function getAppTypeFromImage(image) {
  if (!image) return "Unknown";
  const imgLower = image.toLowerCase();
  if (imgLower.includes("nginx")) return "Static HTML";
  if (imgLower.includes("php") || imgLower.includes("frankenphp")) return "PHP (FrankenPHP)";
  if (imgLower.includes("node")) return "Node.js (Express)";
  if (imgLower.includes("python")) return "Python (FastAPI)";
  return image;
}

export function openAddWebsiteModal() {
  const modal = document.getElementById("add-website-modal");
  if (modal) {
    document.getElementById("web-domain").value = "";
    document.getElementById("web-type").value = "static";
    const chk = document.getElementById("web-create-db");
    if (chk) chk.checked = false;
    const fields = document.getElementById("web-db-fields");
    if (fields) fields.style.display = "none";
    modal.classList.add("active");
  }
}

export function closeAddWebsiteModal() {
  const modal = document.getElementById("add-website-modal");
  if (modal) {
    modal.classList.remove("active");
  }
}

export function closeDeploySuccessModal() {
  const modal = document.getElementById("web-deploy-success-modal");
  if (modal) {
    modal.classList.remove("active");
  }
}

export function toggleWebDbFields() {
  const chk = document.getElementById("web-create-db");
  const fields = document.getElementById("web-db-fields");
  if (!chk || !fields) return;

  if (chk.checked) {
    fields.style.display = "block";
    
    // Load database servers list
    fetch("/api/database/servers")
      .then((res) => res.json())
      .then((res) => {
        const select = document.getElementById("web-db-server");
        if (!select) return;
        select.innerHTML = "";
        
        const servers = Array.isArray(res.data) ? res.data : [];
        if (servers.length > 0) {
          servers.forEach((srv) => {
            const opt = document.createElement("option");
            opt.value = srv.id;
            opt.textContent = `${srv.name} (${srv.driver} - ${srv.host}:${srv.port})`;
            select.appendChild(opt);
          });
        } else {
          const opt = document.createElement("option");
          opt.value = "";
          opt.textContent = "No database servers found. Install one first!";
          select.appendChild(opt);
          chk.checked = false;
          fields.style.display = "none";
          showToast("warning", "Please deploy a Database Server in the Database tab first!");
        }
      })
      .catch((e) => {
        console.error(e);
        showToast("error", "Failed to retrieve database servers list");
      });

    // Auto generate DB credentials based on domain
    const domain = document.getElementById("web-domain").value.trim();
    if (domain) {
      const clean = domain.replace(/[^a-zA-Z0-9]/g, "_").slice(0, 16);
      document.getElementById("web-db-name").value = "db_" + clean;
      document.getElementById("web-db-user").value = "usr_" + clean;
    } else {
      document.getElementById("web-db-name").value = "";
      document.getElementById("web-db-user").value = "";
    }
    document.getElementById("web-db-password").value = generateRandomPassword();
  } else {
    fields.style.display = "none";
  }
}

function generateRandomPassword() {
  const chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  let pass = "";
  for (let i = 0; i < 12; i++) {
    pass += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return pass;
}

// Find a free port starting from 9000 on the client side by probing the system.port_check API
async function findFreePort() {
  for (let port = 9000; port < 10000; port++) {
    try {
      const res = await fetch("/api/system/check_port", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-CSRF-Token": getCSRFToken(),
        },
        body: JSON.stringify({ port: port }),
      });
      const data = await res.json();
      if (data.success && data.data && !data.data.in_use) {
        return port;
      }
    } catch (e) {
      console.error("Error probing port " + port, e);
    }
  }
  throw new Error("No free port available in range 9000-10000");
}

export async function submitAddWebsite() {
  const domainInput = document.getElementById("web-domain");
  const typeSelect = document.getElementById("web-type");

  if (!domainInput || !typeSelect) return;

  const domain = domainInput.value.trim().toLowerCase();
  const appType = typeSelect.value;

  if (!domain) {
    showToast("error", "Domain name is required");
    return;
  }

  // Regex validation for domain format
  if (!/^[a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,8}$/.test(domain)) {
    showToast("error", "Please enter a valid domain name (e.g. example.com)");
    return;
  }

  const createDb = document.getElementById("web-create-db").checked;
  let dbServerId = "";
  let dbName = "";
  let dbUser = "";
  let dbPassword = "";

  if (createDb) {
    dbServerId = document.getElementById("web-db-server").value;
    dbName = document.getElementById("web-db-name").value.trim();
    dbUser = document.getElementById("web-db-user").value.trim();
    dbPassword = document.getElementById("web-db-password").value.trim();

    if (!dbServerId) {
      showToast("error", "Please select a database server");
      return;
    }
    if (!dbName || !dbUser || !dbPassword) {
      showToast("error", "All database fields are required");
      return;
    }
  }

  closeAddWebsiteModal();
  showToast("info", appType === "static" ? `Deploying static website '${domain}'...` : `Scanning free ports and deploying website '${domain}'...`);

  try {
    let allocatedPort = 0;
    if (appType !== "static") {
      allocatedPort = await findFreePort();
    }
    
    const response = await fetch("/api/websites/create", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": getCSRFToken(),
      },
      body: JSON.stringify({
        domain: domain,
        app_type: appType,
        port: allocatedPort,
        container_name: "web_" + domain.replace(/\./g, "_"),
        volume_name: domain.replace(/\./g, "_") + "_app_data",
        create_db: createDb,
        db_server_id: dbServerId,
        db_name: dbName,
        db_user: dbUser,
        db_password: dbPassword,
      }),
    });

    const res = await response.json();
    if (res.success) {
      showToast("success", appType === "static" ? `Website '${domain}' deployed successfully using static hosting` : `Website '${domain}' deployed successfully on port ${allocatedPort}`);
      loadWebsites();

      // Show deployment success details modal
      document.getElementById("success-web-domain").textContent = domain;
      const successDbInfo = document.getElementById("success-db-info");
      
      if (createDb && res.db) {
        successDbInfo.style.display = "block";
        document.getElementById("success-db-host").textContent = res.db.host;
        document.getElementById("success-db-port").textContent = res.db.port;
        document.getElementById("success-db-name").textContent = dbName;
        document.getElementById("success-db-user").textContent = dbUser;
        document.getElementById("success-db-pass").textContent = dbPassword;
      } else {
        successDbInfo.style.display = "none";
      }

      const successModal = document.getElementById("web-deploy-success-modal");
      if (successModal) {
        successModal.classList.add("active");
      }
    } else {
      showToast("error", res.message || "Failed to create website");
    }
  } catch (err) {
    console.error(err);
    showToast("error", err.message || "Network error occurred during deployment");
  }
}

export function deleteWebsite(domain, containerName) {
  if (
    !confirm(
      `Are you sure you want to permanently delete the website '${domain}'?\n\nThis will stop the container, delete the proxy rule, and delete the website's volume/files.`
    )
  ) {
    return;
  }

  showToast("warning", `Deleting website '${domain}'...`);

  fetch("/api/websites/delete", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({
      domain: domain,
      container_name: containerName,
      volume_name: domain.replace(/\./g, "_") + "_app_data",
    }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Website '${domain}' deleted successfully`);
        loadWebsites();
      } else {
        showToast("error", res.message || "Failed to delete website");
      }
    })
    .catch((err) => {
      console.error(err);
      showToast("error", "Network error occurred during deletion");
    });
}

export function toggleWebsite(domain, containerName, currentStatus) {
  const action = currentStatus === "running" ? "stop" : "start";
  showToast("info", `${action === "start" ? "Starting" : "Stopping"} website '${domain}'...`);

  fetch(`/api/containers/${action}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ id: containerName }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Website '${domain}' ${action === "start" ? "started" : "stopped"}`);
        loadWebsites();
      } else {
        showToast("error", res.message || `Failed to ${action} website`);
      }
    })
    .catch((err) => {
      console.error(err);
      showToast("error", "Network error occurred");
    });
}

window.browseWebsiteFiles = function (domain, ruleType) {
  const isStatic = ruleType === "static";
  const path = isStatic ? `/var/www/${domain}` : `/var/lib/zeno-container/volumes/${domain.replace(/\./g, "_")}_app_data`;
  window.currentFilePath = path;
  if (typeof window.loadFilesList === "function") {
    window.loadFilesList(window.currentFilePath);
  }
  switchTab("files");
};

export function openWebSettingsModal(domain, containerName, proxyId) {
  document.getElementById("settings-proxy-id").value = proxyId;
  document.getElementById("settings-proxy-domain").value = domain;
  document.getElementById("settings-proxy-alt-domains").value = "";
  document.getElementById("settings-proxy-volume").value = domain.replace(/\./g, "_") + "_app_data";
  document.getElementById("settings-web-title").textContent = `Website Settings: ${domain}`;
  
  // Reset fields
  document.getElementById("settings-add-domain-input").value = "";
  
  // Switch to Domains tab by default
  switchSettingsTab("domains");
  
  // Show modal
  document.getElementById("web-settings-modal").classList.add("active");
  
  // Load settings details from backend
  loadWebSettingsData(domain, proxyId);
}

export function closeWebSettingsModal() {
  document.getElementById("web-settings-modal").classList.remove("active");
}

export function switchSettingsTab(tabName) {
  // Deactivate all tab buttons
  document.querySelectorAll(".settings-tab-btn").forEach(btn => {
    btn.classList.remove("active");
    btn.style.color = "var(--text-muted)";
  });
  
  // Activate selected tab button
  const activeBtn = document.getElementById(`tab-btn-${tabName}`);
  if (activeBtn) {
    activeBtn.classList.add("active");
    activeBtn.style.color = "#fff";
  }
  
  // Hide all panels
  document.querySelectorAll(".settings-panel").forEach(panel => {
    panel.style.display = "none";
  });
  
  // Show selected panel
  const activePanel = document.getElementById(`settings-panel-${tabName}`);
  if (activePanel) {
    activePanel.style.display = "block";
  }
}

function getProp(obj, propName) {
  if (!obj) return undefined;
  const lower = propName.toLowerCase();
  for (const k of Object.keys(obj)) {
    if (k.toLowerCase() === lower) {
      return obj[k];
    }
  }
  return undefined;
}

async function loadWebSettingsData(domain, proxyId) {
  try {
    const res = await fetch(`/api/websites/settings?proxy_id=${proxyId}&domain=${domain}`);
    const data = await res.json();
    if (!data.success) {
      showToast("error", data.message || "Failed to load website settings");
      return;
    }
    
    // Unwrap from array if necessary
    const rule = Array.isArray(data.rule) ? data.rule[0] : data.rule;
    const db = Array.isArray(data.db) ? data.db[0] : data.db;
    
    console.log("Website Settings Rule Object:", rule);
    
    if (!rule) {
      showToast("error", "No settings found for this website");
      return;
    }

    const ruleDomain = getProp(rule, "domain") || domain;
    const ruleAltDomains = getProp(rule, "alternative_domain") || "";
    
    // 1. Domains Panel
    document.getElementById("settings-proxy-alt-domains").value = ruleAltDomains;
    renderDomainsList(ruleDomain, ruleAltDomains);
    
    // 2. Doc Root Panel
    const isStatic = getProp(rule, "rule_type") === "static";
    const docrootInput = document.getElementById("settings-docroot-path-input");
    const docrootSelectBtn = document.getElementById("settings-docroot-select-btn");
    const docrootSaveBtn = document.getElementById("settings-docroot-save-btn");
    const docrootDesc = document.getElementById("settings-docroot-desc");
    
    const targetPath = getProp(rule, "target") || "";
    const volumeName = domain.replace(/\./g, "_") + "_app_data";
    const defaultHostPath = isStatic ? `/var/www/${domain}` : `/var/lib/zeno-container/volumes/${volumeName}`;
    
    if (isStatic) {
      if (docrootInput) docrootInput.value = targetPath || defaultHostPath;
      if (docrootSelectBtn) docrootSelectBtn.style.display = "inline-flex";
      if (docrootSaveBtn) docrootSaveBtn.style.display = "inline-flex";
      if (docrootDesc) docrootDesc.textContent = "This is the storage directory on the host containing your static files. You can choose a custom directory path using the selector.";
    } else {
      if (docrootInput) docrootInput.value = defaultHostPath;
      if (docrootSelectBtn) docrootSelectBtn.style.display = "none";
      if (docrootSaveBtn) docrootSaveBtn.style.display = "none";
      if (docrootDesc) docrootDesc.textContent = "This is the absolute storage directory on the host where your container files are stored. It is managed automatically.";
    }
    
    // 3. SSL Panel
    const sslEnabled = getProp(rule, "ssl_enabled");
    const sslStatus = getProp(rule, "ssl_status") || "none";
    const sslStatusTextEl = document.getElementById("settings-ssl-status-text");
    const sslBadgeEl = document.getElementById("settings-ssl-status-badge");
    const btnToggleSsl = document.getElementById("btn-toggle-ssl");
    
    if (sslEnabled) {
      let badgeColor = "var(--warning)";
      let badgeLabel = sslStatus.charAt(0).toUpperCase() + sslStatus.slice(1);
      
      if (sslStatus === "active_letsencrypt") {
        badgeColor = "var(--success)";
        badgeLabel = "Active (Let's Encrypt)";
      } else if (sslStatus === "active_self_signed") {
        badgeColor = "#3b82f6";
        badgeLabel = "Active (Self-Signed Fallback)";
      } else if (sslStatus === "failed") {
        badgeColor = "var(--danger)";
        badgeLabel = "Validation Failed";
      }
      
      sslStatusTextEl.innerHTML = `SSL is <strong>Enabled</strong>. Gateway is securing your traffic.`;
      sslBadgeEl.innerHTML = `<span class="badge" style="background:rgba(255,255,255,0.02); color:${badgeColor}; border:1px solid ${badgeColor}; padding:4px 10px; border-radius:12px; font-size:0.8rem; font-weight:600;">${badgeLabel}</span>`;
      btnToggleSsl.innerHTML = `<i class="fa-solid fa-shield-halved"></i> Disable SSL`;
      btnToggleSsl.style.background = "rgba(239, 68, 68, 0.1)";
      btnToggleSsl.style.borderColor = "rgba(239, 68, 68, 0.2)";
      btnToggleSsl.style.color = "var(--danger)";
      btnToggleSsl.dataset.enabled = "true";
    } else {
      sslStatusTextEl.innerHTML = "SSL is currently <strong>Disabled</strong>. Website is served over HTTP.";
      sslBadgeEl.innerHTML = `<span class="badge badge-stopped">Disabled</span>`;
      btnToggleSsl.innerHTML = `<i class="fa-solid fa-shield-halved"></i> Enable SSL (Let's Encrypt)`;
      btnToggleSsl.style.background = "var(--accent-primary)";
      btnToggleSsl.style.borderColor = "var(--accent-primary)";
      btnToggleSsl.style.color = "#fff";
      btnToggleSsl.dataset.enabled = "false";
    }
    
    // 4. Database Panel
    const dbNotLinkedEl = document.getElementById("settings-db-not-linked");
    const dbDetailsEl = document.getElementById("settings-db-details");
    
    if (db) {
      dbNotLinkedEl.style.display = "none";
      dbDetailsEl.style.display = "flex";
      
      document.getElementById("settings-db-host").textContent = getProp(db, "host") || "";
      document.getElementById("settings-db-port").textContent = getProp(db, "port") || "";
      document.getElementById("settings-db-name").textContent = getProp(db, "db_name") || "";
      document.getElementById("settings-db-user").textContent = getProp(db, "db_user") || "";
      document.getElementById("settings-db-pass").textContent = getProp(db, "db_password") || "";
    } else {
      dbNotLinkedEl.style.display = "block";
      dbDetailsEl.style.display = "none";
    }
    
  } catch (err) {
    console.error(err);
    showToast("error", "Error loading settings details");
  }
}

function renderDomainsList(primaryDomain, altDomainsStr) {
  const container = document.getElementById("settings-domains-list");
  container.innerHTML = "";
  
  // 1. Primary domain (cannot be deleted)
  const primRow = document.createElement("div");
  primRow.style.cssText = "display:flex; justify-content:space-between; align-items:center; padding:6px 10px; background:rgba(255,255,255,0.01); border-radius:4px;";
  primRow.innerHTML = `
    <span style="font-family:var(--font-code); font-size:0.88rem; color:#fff;">${primaryDomain}</span>
    <span style="font-size:0.75rem; color:var(--text-muted); font-style:italic;">Primary</span>
  `;
  container.appendChild(primRow);
  
  // 2. Alternative domains
  const alts = altDomainsStr ? altDomainsStr.split(",").map(d => d.trim()).filter(Boolean) : [];
  alts.forEach(domain => {
    const row = document.createElement("div");
    row.style.cssText = "display:flex; justify-content:space-between; align-items:center; padding:6px 10px; background:rgba(255,255,255,0.01); border-radius:4px;";
    row.innerHTML = `
      <span style="font-family:var(--font-code); font-size:0.88rem; color:#fff;">${domain}</span>
      <button class="btn-action" onclick="deleteDomainBinding('${domain}')" style="padding:2px 6px; font-size:0.75rem; color:var(--danger); border-color:rgba(239,68,68,0.2); background:transparent;">
        <i class="fa-solid fa-trash-can"></i>
      </button>
    `;
    container.appendChild(row);
  });
}

export async function addDomainBinding() {
  const proxyId = document.getElementById("settings-proxy-id").value;
  const domain = document.getElementById("settings-proxy-domain").value;
  const inputEl = document.getElementById("settings-add-domain-input");
  const newDomainsStr = inputEl.value.trim().toLowerCase();
  
  if (!newDomainsStr) {
    showToast("error", "Please enter domain name(s) to bind");
    return;
  }
  
  // Split, validate, clean
  const newDomains = newDomainsStr.split(",").map(d => d.trim()).filter(Boolean);
  for (const d of newDomains) {
    if (!/^[a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,8}$/.test(d)) {
      showToast("error", `Invalid domain name format: ${d}`);
      return;
    }
  }
  
  const existingStr = document.getElementById("settings-proxy-alt-domains").value;
  const existingList = existingStr ? existingStr.split(",").map(d => d.trim()).filter(Boolean) : [];
  
  // Merge lists while avoiding duplicates
  const merged = [...new Set([...existingList, ...newDomains])];
  const finalStr = merged.join(",");
  
  showToast("info", "Binding domain(s)...");
  
  try {
    const res = await fetch("/api/websites/settings/update_domains", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": getCSRFToken(),
      },
      body: JSON.stringify({ proxy_id: proxyId, domains: finalStr })
    });
    
    const data = await res.json();
    if (data.success) {
      showToast("success", "Domains bound successfully");
      inputEl.value = "";
      loadWebSettingsData(domain, proxyId);
    } else {
      showToast("error", data.message || "Failed to bind domains");
    }
  } catch (err) {
    console.error(err);
    showToast("error", "Network error occurred");
  }
}

export async function deleteDomainBinding(domainToRemove) {
  if (!confirm(`Unbind domain "${domainToRemove}" from this website?`)) {
    return;
  }
  
  const proxyId = document.getElementById("settings-proxy-id").value;
  const domain = document.getElementById("settings-proxy-domain").value;
  const existingStr = document.getElementById("settings-proxy-alt-domains").value;
  const existingList = existingStr ? existingStr.split(",").map(d => d.trim()).filter(Boolean) : [];
  
  const updatedList = existingList.filter(d => d !== domainToRemove);
  const finalStr = updatedList.join(",");
  
  showToast("info", "Unbinding domain...");
  
  try {
    const res = await fetch("/api/websites/settings/update_domains", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": getCSRFToken(),
      },
      body: JSON.stringify({ proxy_id: proxyId, domains: finalStr })
    });
    
    const data = await res.json();
    if (data.success) {
      showToast("success", "Domain unbound successfully");
      loadWebSettingsData(domain, proxyId);
    } else {
      showToast("error", data.message || "Failed to unbind domain");
    }
  } catch (err) {
    console.error(err);
    showToast("error", "Network error occurred");
  }
}

export async function toggleLetsEncryptSSL() {
  const proxyId = document.getElementById("settings-proxy-id").value;
  const domain = document.getElementById("settings-proxy-domain").value;
  const btnToggleSsl = document.getElementById("btn-toggle-ssl");
  const isEnabled = btnToggleSsl.dataset.enabled === "true";
  
  const nextState = !isEnabled;
  
  showToast("info", nextState ? "Enabling SSL and requesting ACME certificate..." : "Disabling SSL...", 0);
  
  try {
    const res = await fetch("/api/websites/settings/toggle_ssl", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": getCSRFToken(),
      },
      body: JSON.stringify({ proxy_id: proxyId, enabled: nextState })
    });
    
    const data = await res.json();
    
    // Dismiss loading toast
    const activeToast = document.querySelector(".toast-notification");
    if (activeToast) activeToast.remove();
    
    if (data.success) {
      showToast("success", nextState ? "SSL enabled! Falling back to self-signed while Let's Encrypt is pending." : "SSL disabled successfully.");
      loadWebSettingsData(domain, proxyId);
    } else {
      showToast("error", data.message || "Failed to toggle SSL");
    }
  } catch (err) {
    console.error(err);
    showToast("error", "Network error occurred");
  }
}

export function openDocrootInFileManager() {
  const customPath = document.getElementById("settings-docroot-path-input")?.value?.trim();
  const volumeName = document.getElementById("settings-proxy-volume").value;
  const path = customPath || `/var/lib/zeno-container/volumes/${volumeName}`;
  closeWebSettingsModal();
  window.currentFilePath = path;
  window.loadFilesList(window.currentFilePath);
  window.switchTab("files");
  showToast("success", `Opening files: ${path}`);
}

export function openWebDocrootPicker() {
  if (typeof window.openDirPicker === "function") {
    window.openDirPicker("settings-docroot-path-input");
  } else {
    showToast("error", "Directory picker is not loaded");
  }
}

export async function saveWebDocroot() {
  const proxyId = document.getElementById("settings-proxy-id").value;
  const targetPath = document.getElementById("settings-docroot-path-input").value.trim();
  
  if (!targetPath) {
    showToast("warning", "Path cannot be empty");
    return;
  }
  
  showToast("info", "Updating document root...");
  
  try {
    const res = await fetch("/api/websites/settings/update_docroot", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": getCSRFToken()
      },
      body: JSON.stringify({
        proxy_id: proxyId,
        target: targetPath
      })
    });
    const data = await res.json();
    if (data.success) {
      showToast("success", data.message || "Document root updated successfully");
      const domain = document.getElementById("settings-proxy-domain").value;
      loadWebSettingsData(domain, proxyId);
    } else {
      showToast("error", data.message || "Failed to update document root");
    }
  } catch (err) {
    console.error(err);
    showToast("error", "Network error occurred");
  }
}
