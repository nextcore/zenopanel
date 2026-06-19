import { getCSRFToken, escapeHtml } from "./utils.js";
import { showToast } from "./toast.js";

export const containerState = {
  allContainers: [],
  pollingInterval: null,
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

export function renderContainers(containers) {
  const tbody = document.getElementById("container-table-body");
  if (!tbody) return;
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
    const created = container.created_at
      ? new Date(container.created_at + "Z").toLocaleString()
      : "-";

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
  if (containerState.pollingInterval) return;
  loadContainers();
  containerState.pollingInterval = setInterval(loadContainers, 2000);
}

export function stopContainerPolling() {
  if (containerState.pollingInterval) {
    clearInterval(containerState.pollingInterval);
    containerState.pollingInterval = null;
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
        showToast("info", JSON.stringify(res.data, null, 2), 8000);
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

  // Collect env vars
  const envRows = document.querySelectorAll("#container-env-list .env-row");
  const envVars = [];
  envRows.forEach((row) => {
    const key = row.querySelector(".env-key").value.trim();
    const val = row.querySelector(".env-val").value.trim();
    if (key) envVars.push(`${key}=${val}`);
  });

  // Build CLI args
  let cliArgs = `create ${name} --image ${image}`;
  if (cmd) cliArgs += ` --cmd "${cmd}"`;
  if (ports.length > 0) {
    cliArgs += ports.map((p) => ` --port ${p}`).join("");
  }
  if (volumes.length > 0) {
    cliArgs += volumes.map((v) => ` --volume ${v}`).join("");
  }
  if (envVars.length > 0) {
    cliArgs += envVars.map((e) => ` --env ${e}`).join("");
  }

  const body = { cli_args: cliArgs, name };

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
