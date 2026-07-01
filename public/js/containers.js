import { getCSRFToken, escapeHtml } from "./utils.js";
import { showToast } from "./toast.js";

export const containerState = {
  allContainers: [],
  pollingInterval: null,
  eventSource: null,
};

export function loadContainers() {
  fetch("/api/containers/list")
    .then((res) => res.json())
    .then((res) => {
      if (res.data) {
        containerState.allContainers = res.data;
        renderContainers(res.data);
      }
    })
    .catch((err) => {
      console.error("Failed to load containers:", err);
    });
}

let lastContainersJson = "";

export function renderContainers(containers) {
  const tbody = document.getElementById("container-table-body");
  if (!tbody) return;

  const currentJson = JSON.stringify(containers);
  if (currentJson === lastContainersJson) {
    return; // Skip redundant DOM updates if container data is identical
  }
  lastContainersJson = currentJson;

  tbody.innerHTML = "";

  if (!containers || containers.length === 0) {
    tbody.innerHTML = `
            <tr>
                <td colspan="7" style="text-align:center; padding:30px; color:var(--text-muted);">
                    <i class="fa-solid fa-cube" style="font-size:2rem; margin-bottom:10px; display:block; opacity:0.3;"></i>
                    No containers yet.
                    <div style="margin-top:10px;">
                        <button class="btn-primary-action" onclick="openAddContainerModal()" style="font-size:0.85rem;">
                            <i class="fa-solid fa-plus"></i> Create Your First Container
                        </button>
                    </div>
                </td>
            </tr>
        `;
    return;
  }

  containers.forEach((container) => {
    let statusBadge =
      '<span class="status-badge stopped" style="background:rgba(255,255,255,0.05); color:var(--text-muted); border:1px solid var(--card-border); padding:3px 8px; border-radius:4px; font-size:0.75rem;">Stopped</span>';

    if (container.status === "running") {
      statusBadge =
        '<span class="status-badge running" style="background:rgba(16,185,129,0.1); color:var(--success); border:1px solid rgba(16,185,129,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem; display:inline-flex; align-items:center; gap:6px;"><span class="pulse-indicator"></span> Running</span>';
    } else if (container.status === "created") {
      statusBadge =
        '<span class="status-badge starting" style="background:rgba(245,158,11,0.1); color:#f59e0b; border:1px solid rgba(245,158,11,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem;">Created</span>';
    }

    const tr = document.createElement("tr");
    const pidVal = container.pid && container.pid > 0 ? container.pid : "-";
    const portsVal =
      container.ports && container.ports.length > 0
        ? container.ports.join(", ")
        : "-";
    let created = "-";
    if (container.created_at) {
      let dateStr = container.created_at;
      // Append Z only if no timezone indicator is present
      if (!dateStr.endsWith("Z") && !dateStr.includes("+") && (dateStr.match(/-/g) || []).length < 3) {
        dateStr += "Z";
      }
      const d = new Date(dateStr);
      created = isNaN(d.getTime()) ? container.created_at : d.toLocaleString();
    }

    tr.innerHTML = `
            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(container.id)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--accent-primary);">${escapeHtml(container.image)}</td>
            <td>${statusBadge}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${pidVal}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--warning);">${escapeHtml(portsVal)}</td>
            <td style="font-size:0.8rem; color:var(--text-muted);">${created}</td>
            <td style="text-align:right; overflow:visible;">
                <div class="action-dropdown" id="container-dropdown-${container.id}">
                    <button class="action-dropdown-btn" onclick="toggleContainerDropdown(event, '${container.id}')">
                        <i class="fa-solid fa-ellipsis-vertical"></i> Manage
                    </button>
                    <div class="action-dropdown-menu" id="container-menu-${container.id}">
                        ${
                          container.status === "running" ||
                          container.status === "created"
                            ? `
                            <button class="action-dropdown-item" onclick="stopContainer('${container.id}')">
                                <i class="fa-solid fa-stop" style="color:#ef4444;"></i> Stop
                            </button>
                        `
                            : `
                            <button class="action-dropdown-item" onclick="startContainer('${container.id}')">
                                <i class="fa-solid fa-play" style="color:var(--success);"></i> Start
                            </button>
                        `
                        }
                        <button class="action-dropdown-item" onclick="inspectContainer('${container.id}')">
                            <i class="fa-solid fa-magnifying-glass" style="color:var(--accent-primary);"></i> Inspect
                        </button>
                        ${
                          container.ports && container.ports.length > 0
                            ? `
                            <button class="action-dropdown-item" onclick="openContainerProxyModal('${container.id}', '${container.ports[0]}')">
                                <i class="fa-solid fa-network-wired" style="color:#a78bfa;"></i> Add Proxy
                            </button>`
                            : ""
                        }
                        <button class="action-dropdown-item" onclick="browseContainerFiles('${container.id}')">
                            <i class="fa-solid fa-folder-open" style="color:#f59e0b;"></i> Browse Files
                        </button>
                         <button class="action-dropdown-item" onclick="viewContainerLogs('${container.id}')">
                             <i class="fa-solid fa-terminal" style="color:var(--text-main);"></i> Logs
                         </button>
                         <button class="action-dropdown-item" onclick="openContainerTerminal('${container.id}')">
                             <i class="fa-solid fa-code" style="color:var(--success);"></i> Terminal
                         </button>
                         <button class="action-dropdown-item" onclick="openEditContainerResources('${container.id}')">
                             <i class="fa-solid fa-sliders" style="color:var(--accent-primary);"></i> Edit Resources
                         </button>
                         <hr style="border:none; border-top:1px solid var(--card-border); margin:4px 0;">
                         <button class="action-dropdown-item danger" onclick="deleteContainer('${container.id}')">
                             <i class="fa-solid fa-trash-can"></i> Delete
                         </button>
                     </div>
                 </div>
             </td>
         `;
     tbody.appendChild(tr);
   });
}

export function startContainerPolling() {
  if (containerState.eventSource) return;
  
  containerState.eventSource = new EventSource('/api/containers/stream');
  
  containerState.eventSource.onmessage = (event) => {
    try {
      const res = JSON.parse(event.data);
      if (res.data) {
        containerState.allContainers = res.data;
        renderContainers(res.data);
      }
    } catch (e) {
      console.error("Failed to parse container stream:", e);
    }
  };
  
  containerState.eventSource.onerror = (err) => {
    console.error("Container SSE stream error:", err);
  };
}

export function stopContainerPolling() {
  if (containerState.eventSource) {
    containerState.eventSource.close();
    containerState.eventSource = null;
  }
}

// ─── CRUD Actions ───────────────────────────────────────────────────

export function startContainer(id) {
  fetch("/api/containers/start", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ id }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Container ${id} started`);
        setTimeout(loadContainers, 500);
      } else {
        showToast("error", res.message || "Failed to start container");
      }
    })
    .catch((err) => showToast("error", "Network error"));
}

export function stopContainer(id) {
  fetch("/api/containers/stop", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ id }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Container ${id} stopped`);
        setTimeout(loadContainers, 500);
      } else {
        showToast("error", res.message || "Failed to stop container");
      }
    })
    .catch((err) => showToast("error", "Network error"));
}

export function deleteContainer(id) {
  if (!confirm(`Delete container "${id}"? This will permanently remove it.`))
    return;

  fetch("/api/containers/delete", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ id }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Container ${id} removed`);
        setTimeout(loadContainers, 500);
      } else {
        showToast("error", res.message || "Failed to remove container");
      }
    })
    .catch((err) => showToast("error", "Network error"));
}

export function inspectContainer(id) {
  fetch(`/api/containers/inspect?id=${encodeURIComponent(id)}`)
    .then((res) => res.json())
    .then((res) => {
      if (res.success && res.data) {
        let parsedData = res.data;
        if (typeof res.data === "string") {
          try {
            parsedData = JSON.parse(res.data);
          } catch (e) {}
        }
        showToast("info", `<pre style="margin:0; font-family:var(--font-code); font-size:0.8rem; line-height:1.4; color:var(--text-main); white-space:pre-wrap;">${escapeHtml(JSON.stringify(parsedData, null, 2))}</pre>`, 8000);
      } else {
        showToast("error", "Failed to inspect container");
      }
    })
    .catch((err) => showToast("error", "Network error"));
}

export function openContainerProxyModal(id, port) {
  document.getElementById("container-proxy-id").value = id;
  document.getElementById("container-proxy-port").value = port || "";
  document.getElementById("container-proxy-domain").value = "";
  document.getElementById("container-proxy-modal").classList.add("active");
}

export function closeContainerProxyModal() {
  document.getElementById("container-proxy-modal").classList.remove("active");
}

export function submitContainerProxy() {
  const containerId = document.getElementById("container-proxy-id").value;
  const port = document.getElementById("container-proxy-port").value;
  const domain = document.getElementById("container-proxy-domain").value.trim();

  if (!domain) {
    showToast("error", "Domain is required");
    return;
  }
  if (!port) {
    showToast("error", "Port is required");
    return;
  }

  // Parse host port from format like "8080:80"
  const hostPort = port.split(":")[0];
  const target = `http://127.0.0.1:${hostPort}`;

  fetch("/api/containers/add-proxy", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({
      name: `${containerId}-proxy`,
      container_id: containerId,
      domain: domain,
      target: target,
    }),
  })
    .then((res) => res.json())
    .then((res) => {
      closeContainerProxyModal();
      if (res.success) {
        showToast("success", `Proxy rule created: ${domain} → ${target}`);
      } else {
        showToast("error", res.message || "Failed to create proxy rule");
      }
    })
    .catch((err) => {
      closeContainerProxyModal();
      showToast("error", "Network error");
    });
}

// ─── Modal Actions ──────────────────────────────────────────────────

export function openAddContainerModal() {
  document.getElementById("container-id-val").value = "";
  document.getElementById("container-name").value = "";
  document.getElementById("container-image").value = "";
  document.getElementById("container-cmd").value = "";
  document.getElementById("container-memory").value = "";
  document.getElementById("container-cpus").value = "";
  document.getElementById("container-oom-score-adj").value = "";
  document.getElementById("container-read-only").checked = false;

  // Reset cached images list
  document.getElementById("container-cached-images").innerHTML =
    '<button class="btn-action" onclick="loadCachedImagesForCreate()" style="font-size:0.75rem; padding:3px 8px;"><i class="fa-solid fa-layer-group"></i> Load cached</button>';

  // Reset port rows
  const portList = document.getElementById("container-port-list");
  portList.innerHTML = `
        <div class="port-row" style="display:flex; gap:6px; margin-bottom:5px;">
            <input type="text" class="port-host" placeholder="Host" style="width:100px; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
            <span style="align-self:center; color:var(--text-muted);">:</span>
            <input type="text" class="port-container" placeholder="Container" style="width:100px; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
            <button class="btn-action" onclick="this.parentElement.remove()" style="padding:4px 8px; font-size:0.8rem;">✕</button>
        </div>
    `;

  // Reset volume rows
  const volList = document.getElementById("container-volume-list");
  volList.innerHTML = "";

  // Reset env rows
  const envList = document.getElementById("container-env-list");
  envList.innerHTML = `
        <div class="env-row" style="display:flex; gap:6px; margin-bottom:5px;">
            <input type="text" class="env-key" placeholder="KEY" style="width:120px; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
            <input type="text" class="env-val" placeholder="value" style="flex-grow:1; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
            <button class="btn-action" onclick="this.parentElement.remove()" style="padding:4px 8px; font-size:0.8rem;">✕</button>
        </div>
    `;

  document.getElementById("modal-container-title").textContent =
    "Create Container";
  document.getElementById("btn-container-submit").textContent =
    "Create Container";
  document.getElementById("add-container-modal").classList.add("active");
}

export function closeAddContainerModal() {
  document.getElementById("add-container-modal").classList.remove("active");
}

export function loadCachedImagesForCreate() {
  const container = document.getElementById("container-cached-images");
  container.innerHTML =
    '<span style="font-size:0.8rem; color:var(--text-muted);"><i class="fa-solid fa-spinner fa-spin"></i> Loading...</span>';

  fetch("/api/containers/images")
    .then((res) => res.json())
    .then((res) => {
      if (res.data && res.data.length > 0) {
        container.innerHTML =
          '<span style="font-size:0.75rem; color:var(--text-muted); margin-right:4px;">Cached:</span>' +
          res.data
            .map(
              (img) =>
                `<button class="btn-action" onclick="selectCachedImage('${escapeHtml(img)}')" style="font-size:0.75rem; padding:2px 8px;">${escapeHtml(img)}</button>`,
            )
            .join("");
      } else {
        container.innerHTML =
          '<span style="font-size:0.8rem; color:var(--text-muted);">No cached images. Pull one first.</span>';
      }
    })
    .catch(() => {
      container.innerHTML =
        '<span style="font-size:0.8rem; color:#ef4444;">Failed to load.</span>';
    });
}

export function selectCachedImage(name) {
  document.getElementById("container-image").value = name;
  document.getElementById("container-cached-images").innerHTML =
    '<button class="btn-action" onclick="loadCachedImagesForCreate()" style="font-size:0.75rem; padding:3px 8px;"><i class="fa-solid fa-layer-group"></i> Load cached</button>';
}

export function addPortRow() {
  const list = document.getElementById("container-port-list");
  const row = document.createElement("div");
  row.className = "port-row";
  row.style.cssText = "display:flex; gap:6px; margin-bottom:5px;";
  row.innerHTML = `
    <input type="text" class="port-host" placeholder="Host" style="width:100px; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
    <span style="align-self:center; color:var(--text-muted);">:</span>
    <input type="text" class="port-container" placeholder="Container" style="width:100px; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
    <button class="btn-action" onclick="this.parentElement.remove()" style="padding:4px 8px; font-size:0.8rem;">✕</button>
  `;
  list.appendChild(row);
}

export function addVolumeRow() {
  const list = document.getElementById("container-volume-list");
  const row = document.createElement("div");
  row.className = "vol-row";
  row.style.cssText = "display:flex; gap:6px; margin-bottom:5px;";
  row.innerHTML = `
    <input type="text" class="vol-host" placeholder="/host/path" style="width:140px; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
    <span style="align-self:center; color:var(--text-muted);">:</span>
    <input type="text" class="vol-container" placeholder="/container/path" style="flex-grow:1; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
    <button class="btn-action" onclick="this.parentElement.remove()" style="padding:4px 8px; font-size:0.8rem;">✕</button>
  `;
  list.appendChild(row);
}

export function addEnvRow() {
  const envList = document.getElementById("container-env-list");
  const row = document.createElement("div");
  row.className = "env-row";
  row.style.cssText = "display:flex; gap:6px; margin-bottom:5px;";
  row.innerHTML = `
        <input type="text" class="env-key" placeholder="KEY" style="width:120px; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
        <input type="text" class="env-val" placeholder="value" style="flex-grow:1; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(255,255,255,0.03); color:var(--text-main); outline:none; font-family:var(--font-code); font-size:0.85rem;">
        <button class="btn-action" onclick="this.parentElement.remove()" style="padding:4px 8px; font-size:0.8rem;">✕</button>
    `;
  envList.appendChild(row);
}

export function submitAddContainer() {
  const name = document.getElementById("container-name").value.trim();
  const image = document.getElementById("container-image").value.trim();
  const cmd = document.getElementById("container-cmd").value.trim();

  if (!name) {
    showToast("error", "Container name is required");
    return;
  }
  if (!image) {
    showToast("error", "Image name is required");
    return;
  }

  // Collect ports
  const portRows = document.querySelectorAll("#container-port-list .port-row");
  const ports = [];
  portRows.forEach((row) => {
    const host = row.querySelector(".port-host").value.trim();
    const container = row.querySelector(".port-container").value.trim();
    if (host && container) ports.push(`${host}:${container}`);
  });

  // Collect volumes
  const volRows = document.querySelectorAll("#container-volume-list .vol-row");
  const volumes = [];
  volRows.forEach((row) => {
    const host = row.querySelector(".vol-host").value.trim();
    const container = row.querySelector(".vol-container").value.trim();
    if (host && container) volumes.push(`${host}:${container}`);
  });

  // Collect env vars as a structured object
  const envRows = document.querySelectorAll("#container-env-list .env-row");
  const env = {};
  envRows.forEach((row) => {
    const key = row.querySelector(".env-key").value.trim();
    const val = row.querySelector(".env-val").value.trim();
    if (key) env[key] = val;
  });

  const memory = document.getElementById("container-memory").value.trim();
  const cpus = document.getElementById("container-cpus").value.trim();
  const oomScoreAdjVal = document.getElementById("container-oom-score-adj").value.trim();
  const oom_score_adj = oomScoreAdjVal ? parseInt(oomScoreAdjVal, 10) : null;
  const read_only = document.getElementById("container-read-only").checked;

  // Send structured payload
  const body = {
    name,
    image,
    cmd,
    ports,
    volumes,
    env,
    host_net: false,
    memory,
    cpus,
    oom_score_adj,
    read_only
  };

  closeAddContainerModal();
  showToast("info", "Creating container...", 0);

  fetch("/api/containers/create", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify(body),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Container "${name}" created!`);
        const toast = document.querySelector(".toast-notification");
        if (toast) toast.remove();
        setTimeout(loadContainers, 500);
      } else {
        showToast(
          "error",
          res.error || res.message || "Failed to create container",
        );
      }
    })
    .catch((err) => showToast("error", "Network error"));
}

// ─── Images Modal ───────────────────────────────────────────────────

export function openContainerImagesModal() {
  const modal = document.getElementById("container-images-modal");
  const list = document.getElementById("container-images-list");
  list.innerHTML =
    '<div style="text-align:center; padding:20px; color:var(--text-muted);"><i class="fa-solid fa-spinner fa-spin"></i> Loading...</div>';
  modal.classList.add("active");

  fetch("/api/containers/images")
    .then((res) => res.json())
    .then((res) => {
      if (res.data && res.data.length > 0) {
        list.innerHTML = res.data
          .map(
            (img) => `
                    <div style="padding:10px; border-bottom:1px solid var(--card-border); display:flex; align-items:center; gap:10px;">
                        <i class="fa-solid fa-layer-group" style="color:var(--accent-primary);"></i>
                        <span style="flex-grow:1; font-family:var(--font-code); font-size:0.9rem;">${escapeHtml(img)}</span>
                        <button class="btn-action" onclick="deleteImage('${escapeHtml(img)}')" style="padding:4px 8px; font-size:0.8rem; color:#ef4444;">
                            <i class="fa-solid fa-trash-can"></i> Delete
                        </button>
                    </div>
                `,
          )
          .join("");
      } else {
        list.innerHTML =
          '<div style="text-align:center; padding:20px; color:var(--text-muted);">No cached images. Pull one first.</div>';
      }
    })
    .catch((err) => {
      list.innerHTML =
        '<div style="text-align:center; padding:20px; color:#ef4444;">Failed to load images.</div>';
    });
}

export function closeContainerImagesModal() {
  document.getElementById("container-images-modal").classList.remove("active");
}

// ─── Pull Image ─────────────────────────────────────────────────────

export function pullContainerImage() {
  const image = document.getElementById("container-image").value.trim();
  if (!image) {
    showToast("error", "Enter an image name first");
    return;
  }

  const pullModal = document.getElementById("container-pull-modal");
  document.getElementById("pull-status-text").textContent =
    `Pulling ${image}...`;
  document.getElementById("pull-progress-bar").style.width = "30%";
  document.getElementById("pull-detail-text").textContent =
    "Downloading layers...";
  pullModal.classList.add("active");

  fetch("/api/containers/pull", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ image }),
  })
    .then((res) => res.json())
    .then((res) => {
      pullModal.classList.remove("active");
      if (res.success) {
        showToast("success", `Image ${image} pulled successfully`);
      } else {
        showToast("error", res.error || "Failed to pull image");
      }
    })
    .catch((err) => {
      pullModal.classList.remove("active");
      showToast("error", "Network error");
    });
}

// ─── Log Viewer ─────────────────────────────────────────────────────

export function viewContainerLogs(id) {
  const modal = document.getElementById("container-logs-modal");
  const title = document.getElementById("container-logs-title");
  const content = document.getElementById("container-logs-content");
  title.textContent = `Logs: ${id}`;
  content.textContent = "Loading...";
  modal.classList.add("active");

  fetch(`/api/containers/logs?id=${encodeURIComponent(id)}`)
    .then((res) => res.json())
    .then((res) => {
      if (res.data) {
        content.textContent = res.data;
      } else {
        content.textContent = "No logs available.";
      }
    })
    .catch(() => {
      content.textContent = "Failed to load logs.";
    });
}

export function closeContainerLogsModal() {
  document.getElementById("container-logs-modal").classList.remove("active");
}

// ─── Delete Image ───────────────────────────────────────────────────

export function deleteImage(name) {
  if (!confirm(`Delete cached image "${name}"?`)) return;

  fetch("/api/containers/rmi", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ image: name }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Image "${name}" removed`);
        openContainerImagesModal(); // refresh list
      } else {
        showToast("error", res.error || "Failed to remove image");
      }
    })
    .catch((err) => showToast("error", "Network error"));
}

// ─── Sub-tab Switching (Containers / Compose) ───────────────────────

export function switchContainerSubTab(tab) {
  document.querySelectorAll(".sub-tab-btn").forEach((btn) => {
    btn.style.background = "transparent";
    btn.style.color = "var(--text-muted)";
    btn.classList.remove("active");
  });

  const activeBtn = document.getElementById(`subtab-${tab}-btn`);
  if (activeBtn) {
    activeBtn.style.background = "rgba(59,130,246,0.1)";
    activeBtn.style.color = "var(--accent-primary)";
    activeBtn.classList.add("active");
  }

  document.getElementById("subtab-containers").style.display =
    tab === "containers" ? "block" : "none";
  document.getElementById("subtab-compose").style.display =
    tab === "compose" ? "block" : "none";

  if (tab === "compose") {
    import("./compose.js").then((m) => {
      m.loadComposeProjects();
      m.loadComposeYaml();
    });
  }
}

// ─── Browse Container Files ─────────────────────────────────────────

export function browseContainerFiles(id) {
  fetch(`/api/containers/rootfs-path?id=${encodeURIComponent(id)}`)
    .then((res) => res.json())
    .then((res) => {
      if (res.success && res.path) {
        window.currentFilePath = res.path;
        window.loadFilesList(res.path);
        window.switchTab("files");
        showToast("success", `Browsing container: ${id}`);
      } else {
        showToast("error", "Failed to get container path");
      }
    })
    .catch((err) => showToast("error", "Network error"));
}

// ─── Dropdown Toggle ───────────────────────────────────────────────

export function toggleContainerDropdown(event, id) {
  event.stopPropagation();

  // Pause polling while dropdown is open
  const anyOpen =
    document.querySelectorAll(".action-dropdown-menu.show").length > 0;

  const targetMenu = document.getElementById(`container-menu-${id}`);
  const allMenus = document.querySelectorAll(".action-dropdown-menu");
  allMenus.forEach((menu) => {
    if (menu !== targetMenu) {
      menu.classList.remove("show");
      menu.style.position = "";
      menu.style.top = "";
      menu.style.left = "";
      menu.style.right = "";
      menu.style.bottom = "";
      if (menu.parentElement) {
        menu.parentElement.classList.remove("open-up");
      }
    }
  });

  const isShowing = targetMenu.classList.contains("show");
  if (isShowing) {
    targetMenu.classList.remove("show");
    // Resume polling
    if (!document.querySelector(".action-dropdown-menu.show")) {
      startContainerPolling();
    }
    return;
  }

  // Close all dropdowns and pause polling
  stopContainerPolling();

  targetMenu.classList.add("show");
  targetMenu.style.position = "fixed";

  const btn = event.currentTarget;
  const btnRect = btn.getBoundingClientRect();
  const menuWidth = 220;
  const menuHeight = targetMenu.offsetHeight || 200;
  const spaceBelow = window.innerHeight - btnRect.bottom;
  const spaceAbove = btnRect.top;

  let leftPos = btnRect.right - menuWidth;
  if (leftPos < 10) leftPos = 10;

  targetMenu.style.left = `${leftPos}px`;
  targetMenu.style.right = "auto";
  targetMenu.style.top = `${btnRect.bottom + 4}px`;
  targetMenu.style.bottom = "auto";

  if (spaceBelow < menuHeight && spaceAbove > menuHeight) {
    targetMenu.style.top = "auto";
    targetMenu.style.bottom = `${window.innerHeight - btnRect.top + 4}px`;
    targetMenu.parentElement.classList.add("open-up");
  } else {
    targetMenu.parentElement.classList.remove("open-up");
  }
}

// ─── Resource Updates ──────────────────────────────────────────────

export function openEditContainerResources(id) {
  const container = containerState.allContainers.find((c) => c.id === id);
  if (!container) return;

  document.getElementById("edit-container-resources-id").value = id;
  document.getElementById("edit-container-resources-name").textContent = id;

  let memStr = "";
  if (container.memory_limit && container.memory_limit > 0) {
    const bytes = container.memory_limit;
    if (bytes % (1024 * 1024 * 1024) === 0) {
      memStr = (bytes / (1024 * 1024 * 1024)) + "g";
    } else {
      memStr = (bytes / (1024 * 1024)) + "m";
    }
  }
  document.getElementById("edit-container-resources-memory").value = memStr;

  let cpuStr = "";
  if (container.cpu_limit && container.cpu_limit > 0) {
    cpuStr = container.cpu_limit.toString();
  }
  document.getElementById("edit-container-resources-cpus").value = cpuStr;

  document.getElementById("edit-container-resources-modal").classList.add("active");
}

export function closeEditContainerResourcesModal() {
  document.getElementById("edit-container-resources-modal").classList.remove("active");
}

export function submitEditContainerResources() {
  const id = document.getElementById("edit-container-resources-id").value;
  const memory = document.getElementById("edit-container-resources-memory").value.trim();
  const cpus = document.getElementById("edit-container-resources-cpus").value.trim();

  const btn = document.getElementById("btn-update-resources-submit");
  const origText = btn.innerHTML;
  btn.disabled = true;
  btn.innerHTML = `<i class="fa-solid fa-spinner fa-spin"></i> Saving...`;

  fetch("/api/containers/update", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ id, memory, cpus }),
  })
    .then((res) => res.json())
    .then((res) => {
      btn.disabled = false;
      btn.innerHTML = origText;
      if (res.success) {
        showToast("success", `Container resources updated successfully`);
        closeEditContainerResourcesModal();
        loadContainers();
      } else {
        showToast("error", res.error || res.message || "Failed to update resources");
      }
    })
    .catch((err) => {
      btn.disabled = false;
      btn.innerHTML = origText;
      showToast("error", "Network error");
    });
}

// ─── Volumes Management ──────────────────────────────────────────────

export function loadVolumes() {
  const tbody = document.getElementById("volume-table-body");
  if (!tbody) return;
  tbody.innerHTML = '<tr><td colspan="4" style="text-align:center; padding:20px; color:var(--text-muted);"><i class="fa-solid fa-spinner fa-spin"></i> Loading volumes...</td></tr>';

  fetch("/api/volumes/list")
    .then((res) => res.json())
    .then((res) => {
      tbody.innerHTML = "";
      if (res.success && res.data && res.data.length > 0) {
        res.data.forEach((vol) => {
          const tr = document.createElement("tr");
          tr.innerHTML = `
            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(vol.Name)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${escapeHtml(vol.Driver)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--text-muted);">${escapeHtml(vol.Mountpoint)}</td>
            <td style="text-align:right;">
              <button class="btn-action" onclick="deleteVolume('${escapeHtml(vol.Name)}')" style="color:#ef4444; padding:4px 8px; font-size:0.8rem;">
                <i class="fa-solid fa-trash-can"></i> Delete
              </button>
            </td>
          `;
          tbody.appendChild(tr);
        });
      } else {
        tbody.innerHTML = '<tr><td colspan="4" style="text-align:center; padding:30px; color:var(--text-muted);">No volumes found.</td></tr>';
      }
    })
    .catch(() => {
      tbody.innerHTML = '<tr><td colspan="4" style="text-align:center; padding:20px; color:#ef4444;">Failed to load volumes.</td></tr>';
    });
}

export function openAddVolumeModal() {
  document.getElementById("volume-name").value = "";
  document.getElementById("add-volume-modal").classList.add("active");
}

export function closeAddVolumeModal() {
  document.getElementById("add-volume-modal").classList.remove("active");
}

export function submitAddVolume() {
  const name = document.getElementById("volume-name").value.trim();
  if (!name) {
    showToast("error", "Volume name is required");
    return;
  }
  closeAddVolumeModal();
  showToast("info", "Creating volume...");

  fetch("/api/volumes/create", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ name }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Volume "${name}" created`);
        loadVolumes();
      } else {
        showToast("error", res.message || "Failed to create volume");
      }
    })
    .catch(() => showToast("error", "Network error"));
}

export function deleteVolume(name) {
  if (!confirm(`Delete volume "${name}"? This will permanently delete the folder.`)) return;
  showToast("info", "Deleting volume...");

  fetch("/api/volumes/delete", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ name }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Volume "${name}" removed`);
        loadVolumes();
      } else {
        showToast("error", res.message || "Failed to remove volume");
      }
    })
    .catch(() => showToast("error", "Network error"));
}

// ─── Networks Management ─────────────────────────────────────────────

export function loadNetworks() {
  const tbody = document.getElementById("network-table-body");
  if (!tbody) return;
  tbody.innerHTML = '<tr><td colspan="6" style="text-align:center; padding:20px; color:var(--text-muted);"><i class="fa-solid fa-spinner fa-spin"></i> Loading networks...</td></tr>';

  fetch("/api/networks/list")
    .then((res) => res.json())
    .then((res) => {
      tbody.innerHTML = "";
      if (res.success && res.data && res.data.length > 0) {
        res.data.forEach((net) => {
          const tr = document.createElement("tr");
          tr.innerHTML = `
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--accent-primary);">${escapeHtml(net.Id)}</td>
            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(net.Name)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${escapeHtml(net.Driver)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--warning);">${escapeHtml(net.Subnet || "-")}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--success);">${escapeHtml(net.Gateway || "-")}</td>
            <td style="text-align:right;">
              ${net.Id === "zenobr0" ? 
                `<span style="color:var(--text-muted); font-size:0.8rem; font-style:italic; padding-right:10px;">Default System</span>` : 
                `<button class="btn-action" onclick="deleteNetwork('${escapeHtml(net.Name)}')" style="color:#ef4444; padding:4px 8px; font-size:0.8rem;">
                  <i class="fa-solid fa-trash-can"></i> Delete
                </button>`
              }
            </td>
          `;
          tbody.appendChild(tr);
        });
      } else {
        tbody.innerHTML = '<tr><td colspan="6" style="text-align:center; padding:30px; color:var(--text-muted);">No networks found.</td></tr>';
      }
    })
    .catch(() => {
      tbody.innerHTML = '<tr><td colspan="6" style="text-align:center; padding:20px; color:#ef4444;">Failed to load networks.</td></tr>';
    });
}

export function openAddNetworkModal() {
  document.getElementById("network-name").value = "";
  document.getElementById("add-network-modal").classList.add("active");
}

export function closeAddNetworkModal() {
  document.getElementById("add-network-modal").classList.remove("active");
}

export function submitAddNetwork() {
  const name = document.getElementById("network-name").value.trim();
  if (!name) {
    showToast("error", "Network name is required");
    return;
  }
  closeAddNetworkModal();
  showToast("info", "Creating network...");

  fetch("/api/networks/create", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ name }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Network "${name}" created (mocked)`);
        loadNetworks();
      } else {
        showToast("error", res.message || "Failed to create network");
      }
    })
    .catch(() => showToast("error", "Network error"));
}

export function deleteNetwork(name) {
  if (!confirm(`Delete network "${name}"?`)) return;
  showToast("info", "Deleting network...");

  fetch("/api/networks/delete", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": getCSRFToken(),
    },
    body: JSON.stringify({ name }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Network "${name}" removed`);
        loadNetworks();
      } else {
        showToast("error", res.message || "Failed to remove network");
      }
    })
    .catch(() => showToast("error", "Network error"));
}

export function openContainerTerminal(id) {
  window.pendingContainerTerminalId = id;
  window.switchTab("terminal");
}

