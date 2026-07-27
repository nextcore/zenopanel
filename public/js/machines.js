import { getCSRFToken, formatBytes, escapeHtml } from "./utils.js";
import { showToast } from "./toast.js";
import { runMachinePreflight, startMachineMigrationProgress } from "./migration.js";

export function loadPendingMigrationRequests() {
    const container = document.getElementById("pending-migration-container");
    if (!container) return;

    fetch("/api/machines/migration-requests")
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data && res.data.length > 0) {
                let html = "";
                res.data.forEach(req => {
                    html += `
                        <div style="background: rgba(245, 158, 11, 0.12); border: 1px solid rgba(245, 158, 11, 0.4); border-radius: 12px; padding: 16px; margin-bottom: 12px; color: var(--text-main); display: flex; justify-content: space-between; align-items: center; box-shadow: 0 0 15px rgba(245, 158, 11, 0.15);">
                            <div>
                                <div style="color: #f59e0b; font-weight: 700; font-size: 0.95rem; display: flex; align-items: center; gap: 8px;">
                                    <i class="fa-solid fa-bell fa-bounce"></i> Permintaan Live Migration Masuk!
                                </div>
                                <div style="font-size: 0.85rem; margin-top: 4px;">
                                    Server <strong>${req.source_host}</strong> ingin memindahkan Zeno Machine <strong>${req.machine_name}</strong> (${req.vcpus} vCPU / ${req.memory_mb} MB RAM) ke server ini.
                                </div>
                            </div>
                            <div style="display: flex; gap: 8px;">
                                <button class="btn btn-sm btn-success" onclick="acceptMigrationRequest(${req.id})" style="background: #10b981; color: white; border: none; padding: 8px 16px; font-weight: 600; cursor: pointer; border-radius: 6px;">
                                    <i class="fa-solid fa-check"></i> Accept & Receive
                                </button>
                                <button class="btn btn-sm btn-danger" onclick="rejectMigrationRequest(${req.id})" style="background: rgba(239, 68, 68, 0.8); color: white; border: none; padding: 8px 14px; cursor: pointer; border-radius: 6px;">
                                    <i class="fa-solid fa-xmark"></i> Reject
                                </button>
                            </div>
                        </div>
                    `;
                });
                container.innerHTML = html;
            } else {
                container.innerHTML = "";
            }
        })
        .catch(() => {
            if (container) container.innerHTML = "";
        });
}

export function acceptMigrationRequest(id) {
    const csrf = getCSRFToken();
    fetch("/api/machines/migration-requests/accept", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message);
            loadPendingMigrationRequests();
            loadZenoMachines();
        } else {
            showToast("error", res.message);
        }
    });
}

export function rejectMigrationRequest(id) {
    const csrf = getCSRFToken();
    fetch("/api/machines/migration-requests/reject", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message);
            loadPendingMigrationRequests();
        } else {
            showToast("error", res.message);
        }
    });
}

export function loadZenoMachines() {
    loadPendingMigrationRequests();
    const tbody = document.getElementById("machines-table-body");
    if (!tbody) return;

    fetch("/api/machines/list")
        .then(res => res.json())
        .then(res => {
            if (!res.success) {
                tbody.innerHTML = `<tr><td colspan="6" style="padding: 20px; text-align: center; color: var(--danger);">Gagal memuat Zeno Machines</td></tr>`;
                return;
            }

            const machines = res.data || [];
            const stats = res.stats || {};

            // Render stats
            document.getElementById("stat-total-machines").textContent = machines.length;
            document.getElementById("stat-running-machines").textContent = stats.running_count || 0;
            document.getElementById("stat-allocated-vcpus").textContent = `${stats.running_vcpus || 0} Cores`;
            document.getElementById("stat-allocated-ram").textContent = `${stats.running_ram || 0} MB`;

            if (machines.length === 0) {
                tbody.innerHTML = `
                    <tr>
                        <td colspan="7" style="padding: 40px; text-align: center; color: var(--text-muted);">
                            <i class="fa-solid fa-server" style="font-size: 2rem; margin-bottom: 12px; opacity: 0.5;"></i>
                            <div>Belum ada Zeno Machine yang dibuat.</div>
                            <div style="font-size: 0.78rem; margin-top: 4px;">Klik "New Zeno Machine" untuk membuat MicroVM berbasis Cloud-Hypervisor pertama Anda.</div>
                        </td>
                    </tr>
                `;
                return;
            }

            let html = "";
            window.allZenoMachines = machines;

            machines.forEach(m => {
                const isRunning = m.status === "running";
                const isPaused = m.status === "paused";
                const statusBadge = isRunning
                    ? `<span style="background: rgba(16, 185, 129, 0.15); color: #10b981; border: 1px solid rgba(16, 185, 129, 0.3); padding: 4px 10px; border-radius: 20px; font-size: 0.75rem; font-weight: 600;">● Running</span>`
                    : isPaused
                    ? `<span style="background: rgba(245, 158, 11, 0.15); color: #f59e0b; border: 1px solid rgba(245, 158, 11, 0.3); padding: 4px 10px; border-radius: 20px; font-size: 0.75rem; font-weight: 600;">⏸ Paused</span>`
                    : `<span style="background: rgba(239, 68, 68, 0.15); color: #ef4444; border: 1px solid rgba(239, 68, 68, 0.3); padding: 4px 10px; border-radius: 20px; font-size: 0.75rem; font-weight: 600;">○ Stopped</span>`;

                const osIcon = m.os_type === "windows"
                    ? `<i class="fa-brands fa-windows" style="color: #0078d4;"></i> Windows`
                    : `<i class="fa-brands fa-linux" style="color: #f59e0b;"></i> Linux`;

                const isoBadge = m.iso_path
                    ? `<div style="font-size: 0.72rem; color: #ec4899; margin-top: 4px; display: flex; align-items: center; gap: 6px;">
                        <i class="fa-solid fa-compact-disc"></i> ISO: ${m.iso_path.split('/').pop()}
                        <button onclick="detachIsoFromMachine('${m.name}')" title="Detach ISO" style="background: none; border: none; color: #ef4444; cursor: pointer; padding: 0; font-size: 0.72rem;"><i class="fa-solid fa-link-slash"></i></button>
                       </div>`
                    : '';

                html += `
                    <tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.05);">
                        <td style="padding: 14px 20px; font-weight: 600; color: var(--text-main);">
                            <div style="display: flex; align-items: center; gap: 8px;">
                                <i class="fa-solid fa-server" style="color: var(--accent-primary);"></i>
                                ${m.name}
                            </div>
                            <div style="font-size: 0.72rem; color: #10b981; margin-top: 4px; display: flex; align-items: center; gap: 4px;" title="DNS Internal VM">
                                <i class="fa-solid fa-globe"></i> ${m.name}.zeno
                            </div>
                            ${isoBadge}
                        </td>
                        <td style="padding: 14px 16px; color: var(--text-muted);">${osIcon}</td>
                        <td style="padding: 14px 16px; color: var(--text-main); font-weight: 500;">
                            ${m.vcpus} vCPU / ${m.memory_mb} MB RAM
                        </td>
                        <td style="padding: 14px 16px; color: var(--text-muted);">${m.disk_size_gb} GB</td>
                        <td style="padding: 14px 16px;">${statusBadge}</td>
                        <td style="padding: 14px 16px;">
                            ${isRunning ? `
                                <div style="display:flex; align-items:center; gap:8px;">
                                    <div style="display:flex; flex-direction:column; align-items:center;">
                                        <canvas id="sparkline-vm-cpu-${m.name}" width="60" height="20" style="width:60px; height:20px;" title="CPU Usage"></canvas>
                                        <span style="font-size:0.6rem; color:var(--text-muted); margin-top:2px; font-weight:600; letter-spacing:0.5px;">CPU</span>
                                    </div>
                                    <div style="display:flex; flex-direction:column; align-items:center;">
                                        <canvas id="sparkline-vm-ram-${m.name}" width="60" height="20" style="width:60px; height:20px;" title="RAM Usage"></canvas>
                                        <span style="font-size:0.6rem; color:var(--text-muted); margin-top:2px; font-weight:600; letter-spacing:0.5px;">RAM</span>
                                    </div>
                                </div>
                            ` : '<span style="font-size:0.75rem; color:var(--text-muted);">—</span>'}
                        </td>
                        <td style="padding: 14px 20px; text-align: right;">
                            <div style="display: flex; justify-content: flex-end; gap: 6px;">
                                ${
                                    isRunning
                                        ? `<button class="btn btn-secondary btn-sm" onclick="stopMachine('${m.name}')" title="Stop Machine"><i class="fa-solid fa-stop" style="color: #ef4444;"></i></button>`
                                        : `<button class="btn btn-secondary btn-sm" onclick="startMachine('${m.name}')" title="Start Machine"><i class="fa-solid fa-play" style="color: #10b981;"></i></button>`
                                }
                                <button class="btn btn-secondary btn-sm" onclick="viewMachineBootLog('${m.name}')" title="View Boot Log"><i class="fa-solid fa-clock-rotate-left" style="color: #10b981;"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="openMachineConsoleModal('${m.name}')" title="Web Serial Console"><i class="fa-solid fa-terminal" style="color: #38bdf8;"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="openMachineProxyModal('${m.name}', '${m.ip_address}')" title="1-Click Expose via Reverse Proxy"><i class="fa-solid fa-network-wired" style="color: var(--accent-primary);"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="openSnapshotManagerModal('${m.name}')" title="Snapshot Manager"><i class="fa-solid fa-camera" style="color: #a855f7;"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="openResizeMachineModal('${m.name}', ${m.vcpus}, ${m.memory_mb}, ${m.disk_size_gb})" title="Live Resize vCPU & RAM"><i class="fa-solid fa-sliders" style="color: #f59e0b;"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="openMigrateMachineModal('${m.name}')" title="Live Migration"><i class="fa-solid fa-arrows-rotate" style="color: #8b5cf6;"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="deleteMachine('${m.name}')" title="Delete Machine"><i class="fa-solid fa-trash" style="color: var(--text-muted);"></i></button>
                            </div>
                        </td>
                    </tr>
                `;
            });

            tbody.innerHTML = html;
        })
        .catch(err => {
            console.error("Error loading machines:", err);
            tbody.innerHTML = `<tr><td colspan="7" style="padding: 20px; text-align: center; color: var(--danger);">Network error loading machines</td></tr>`;
        });
}

export function populateIsoSelectDropdown() {
    const select = document.getElementById("machine-iso-input");
    if (!select) return;

    fetch("/api/machines/isos/list")
        .then(res => res.json())
        .then(res => {
            const data = res.data || (Array.isArray(res.success) ? res.success : []);
            const isSuccess = res.success === true || Array.isArray(res.success);

            if (isSuccess) {
                let html = `<option value="">-- No ISO Attached (Standard Boot) --</option>`;
                data.forEach(iso => {
                    html += `<option value="${iso.path}">📀 ${iso.name}</option>`;
                });
                select.innerHTML = html;
            }
        })
        .catch(() => {});
}

export function toggleBootSource(type) {
    const isoWrapper = document.getElementById("machine-boot-source-iso-wrapper");
    const ociWrapper = document.getElementById("machine-boot-source-oci-wrapper");
    if (type === "iso") {
        if (isoWrapper) isoWrapper.style.display = "block";
        if (ociWrapper) ociWrapper.style.display = "none";
    } else {
        if (isoWrapper) isoWrapper.style.display = "none";
        if (ociWrapper) ociWrapper.style.display = "block";
    }
}
window.toggleBootSource = toggleBootSource;

export function onOciDistroChange(val) {
    const customWrapper = document.getElementById("machine-oci-custom-wrapper");
    const versionWrapper = document.getElementById("machine-oci-version-wrapper");
    const versionSelect = document.getElementById("machine-oci-version-select");

    if (val === "custom") {
        if (customWrapper) customWrapper.style.display = "block";
        if (versionWrapper) versionWrapper.style.display = "none";
        const input = document.getElementById("machine-oci-image-input");
        if (input) {
            input.value = "";
            input.focus();
        }
    } else {
        if (customWrapper) customWrapper.style.display = "none";
        if (versionWrapper) versionWrapper.style.display = "block";
        
        // Fetch versions from Docker Hub API
        if (versionSelect) {
            versionSelect.innerHTML = `<option value="">Loading versions...</option>`;
            fetch(`/api/containers/docker-tags?repo=${encodeURIComponent(val)}`)
                .then(res => res.json())
                .then(res => {
                    const tags = res.data || [];
                    if (tags.length > 0) {
                        let html = "";
                        tags.forEach(tag => {
                            if (tag.name && tag.name.length < 30 && !tag.name.includes("sha256")) {
                                html += `<option value="${tag.name}">${tag.name}</option>`;
                            }
                        });
                        versionSelect.innerHTML = html;
                    } else {
                        versionSelect.innerHTML = `<option value="latest">latest</option>`;
                    }
                })
                .catch(() => {
                    versionSelect.innerHTML = `<option value="latest">latest (Fetch failed)</option>`;
                });
        }
    }
}
window.onOciDistroChange = onOciDistroChange;

export function loadCachedOciImages() {
    const container = document.getElementById("machine-oci-cached-images");
    if (!container) return;

    container.innerHTML = '<span style="font-size: 0.78rem; color: var(--text-muted);"><i class="fa-solid fa-spinner fa-spin"></i> Memuat daftar image...</span>';

    fetch("/api/containers/images")
        .then(res => res.json())
        .then(res => {
            if (res.data && res.data.length > 0) {
                container.innerHTML =
                    '<div style="font-size: 0.72rem; color: var(--text-muted); margin-bottom: 6px;">Cached images (klik untuk memilih):</div>' +
                    '<div style="display: flex; flex-wrap: wrap; gap: 5px;">' +
                    res.data.map(img =>
                        `<button type="button" class="btn-action" onclick="window.selectOciImage('${img.replace(/'/g, "\\'")}')" style="font-size: 0.73rem; padding: 3px 8px; border-radius: 4px;"><i class="fa-solid fa-cube" style="margin-right: 3px; opacity: 0.7;"></i>${img}</button>`
                    ).join("") +
                    '</div>';
            } else {
                container.innerHTML =
                    '<div style="font-size: 0.78rem; color: var(--text-muted); padding: 6px 0;">' +
                    '<i class="fa-solid fa-inbox" style="opacity: 0.5; margin-right: 4px;"></i>' +
                    'Belum ada image yang di-cache. Pull image terlebih dahulu di tab Containers.' +
                    '</div>' +
                    '<button type="button" class="btn-action" onclick="window.loadCachedOciImages()" style="font-size: 0.73rem; padding: 3px 8px; margin-top: 4px;"><i class="fa-solid fa-rotate"></i> Refresh</button>';
            }
        })
        .catch(() => {
            container.innerHTML = '<span style="font-size: 0.78rem; color: var(--danger);"><i class="fa-solid fa-triangle-exclamation"></i> Gagal memuat daftar image.</span>';
        });
}
window.loadCachedOciImages = loadCachedOciImages;

export function selectOciImage(name) {
    const input = document.getElementById("machine-oci-image-input");
    if (input) {
        input.value = name;
        input.focus();
    }
}
window.selectOciImage = selectOciImage;

// ── Docker Hub Autocomplete Search ──────────────────────────────────
let _ociSearchTimer = null;
let _ociSearchAbort = null;

export function onOciImageSearch(query) {
    const dropdown = document.getElementById("machine-oci-autocomplete");
    const icon = document.getElementById("machine-oci-search-icon");
    if (!dropdown) return;

    // Clear previous timer
    if (_ociSearchTimer) clearTimeout(_ociSearchTimer);
    if (_ociSearchAbort) _ociSearchAbort.abort();

    const q = (query || "").trim();

    // If query contains ":" (user is typing tag), hide autocomplete
    if (q.includes(":") || q.length < 2) {
        dropdown.style.display = "none";
        return;
    }

    // Debounce 400ms
    _ociSearchTimer = setTimeout(() => {
        if (icon) {
            icon.className = "fa-solid fa-spinner fa-spin";
            icon.style.color = "var(--text-muted)";
        }

        _ociSearchAbort = new AbortController();
        fetch(`/api/containers/docker-search?q=${encodeURIComponent(q)}`, { signal: _ociSearchAbort.signal })
            .then(res => res.json())
            .then(res => {
                if (icon) {
                    icon.className = "fa-brands fa-docker";
                    icon.style.color = "#2496ed";
                }

                if (!res.data || res.data.length === 0) {
                    dropdown.innerHTML = `<div style="padding: 12px; text-align: center; color: var(--text-muted); font-size: 0.8rem;">
                        <i class="fa-solid fa-search" style="opacity: 0.4;"></i> Tidak ditemukan image untuk "<strong>${q}</strong>"
                    </div>`;
                    dropdown.style.display = "block";
                    return;
                }

                let html = "";
                res.data.forEach(img => {
                    const name = img.name || "";
                    const desc = (img.description || "").substring(0, 80);
                    const stars = img.stars || 0;
                    const isOfficial = img.is_official || false;

                    const officialBadge = isOfficial
                        ? `<span style="background: #2496ed; color: white; font-size: 0.65rem; padding: 1px 5px; border-radius: 3px; margin-left: 6px; font-weight: 600;">Official</span>`
                        : "";
                    const starBadge = stars > 0
                        ? `<span style="color: #f59e0b; font-size: 0.72rem; margin-left: 6px;"><i class="fa-solid fa-star"></i> ${stars >= 1000 ? (stars / 1000).toFixed(1) + "k" : stars}</span>`
                        : "";

                    html += `<div class="oci-autocomplete-item" onclick="window.selectOciImage('${name.replace(/'/g, "\\'")}'); document.getElementById('machine-oci-autocomplete').style.display='none';"
                        style="padding: 8px 12px; cursor: pointer; border-bottom: 1px solid rgba(255,255,255,0.06); transition: background 0.15s;"
                        onmouseenter="this.style.background='rgba(99,102,241,0.15)'"
                        onmouseleave="this.style.background='transparent'">
                        <div style="display: flex; align-items: center;">
                            <i class="fa-brands fa-docker" style="color: #2496ed; margin-right: 8px; font-size: 0.9rem;"></i>
                            <span style="color: var(--text-main); font-size: 0.84rem; font-weight: 500; font-family: var(--font-code);">${name}</span>
                            ${officialBadge}${starBadge}
                        </div>
                        ${desc ? `<div style="color: var(--text-muted); font-size: 0.72rem; margin-top: 2px; margin-left: 24px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">${desc}</div>` : ""}
                    </div>`;
                });

                dropdown.innerHTML = html;
                dropdown.style.display = "block";
            })
            .catch(err => {
                if (err.name === "AbortError") return;
                if (icon) {
                    icon.className = "fa-brands fa-docker";
                    icon.style.color = "#2496ed";
                }
                dropdown.style.display = "none";
            });
    }, 400);
}
window.onOciImageSearch = onOciImageSearch;

// Close autocomplete when clicking outside
document.addEventListener("click", (e) => {
    const dropdown = document.getElementById("machine-oci-autocomplete");
    const input = document.getElementById("machine-oci-image-input");
    if (dropdown && input && !input.contains(e.target) && !dropdown.contains(e.target)) {
        dropdown.style.display = "none";
    }
});

export function openCreateMachineModal() {
    const modal = document.getElementById("create-machine-modal");
    if (modal) {
        document.getElementById("machine-name-input").value = "";
        const sshInput = document.getElementById("machine-ssh-key-input");
        if (sshInput) sshInput.value = "";
        const pwdInput = document.getElementById("machine-password-input");
        if (pwdInput) pwdInput.value = "";
        
        // Reset Boot Source to ISO
        const isoRadio = document.querySelector('input[name="machine-boot-source-type"][value="iso"]');
        if (isoRadio) isoRadio.checked = true;
        toggleBootSource("iso");

        const ociDistroSelect = document.getElementById("machine-oci-distro-select");
        if (ociDistroSelect) {
            ociDistroSelect.value = "library/alpine";
            onOciDistroChange("library/alpine");
        }

        populateIsoSelectDropdown();
        modal.style.display = "flex";
    }
}

export function closeCreateMachineModal() {
    const modal = document.getElementById("create-machine-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function submitCreateMachine() {
    const name = document.getElementById("machine-name-input").value.trim();
    const os_type = document.getElementById("machine-os-input").value;
    
    // Determine boot source type (iso or oci)
    const bootSourceType = document.querySelector('input[name="machine-boot-source-type"]:checked')?.value || "iso";
    let iso_path = "";
    if (bootSourceType === "oci") {
        const distro = document.getElementById("machine-oci-distro-select")?.value || "library/alpine";
        let ociImg = "";
        if (distro === "custom") {
            ociImg = document.getElementById("machine-oci-image-input")?.value.trim() || "";
        } else {
            const version = document.getElementById("machine-oci-version-select")?.value || "latest";
            const cleanRepo = distro.startsWith("library/") ? distro.substring(8) : distro;
            ociImg = `${cleanRepo}:${version}`;
        }
        if (!ociImg) {
            showToast("error", "Nama OCI Container image wajib diisi");
            return;
        }
        iso_path = `oci://${ociImg}`;
    } else {
        iso_path = document.getElementById("machine-iso-input")?.value || "";
    }

    const vcpus = parseInt(document.getElementById("machine-vcpu-input").value, 10);
    const memory_mb = parseInt(document.getElementById("machine-ram-input").value, 10);
    const disk_size_gb = parseInt(document.getElementById("machine-disk-input").value, 10);
    const ssh_key = (document.getElementById("machine-ssh-key-input")?.value || "").trim();
    const root_password = (document.getElementById("machine-password-input")?.value || "").trim();

    if (!name) {
        showToast("error", "Nama machine wajib diisi");
        return;
    }

    const csrf = getCSRFToken();
    fetch("/api/machines/create", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name, os_type, vcpus, memory_mb, disk_size_gb, ssh_key, root_password, iso_path })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || "Zeno Machine berhasil dibuat");
            closeCreateMachineModal();
            loadZenoMachines();
        } else {
            showToast("error", res.message || "Gagal membuat Zeno Machine");
        }
    })
    .catch(err => {
        console.error("Error creating machine:", err);
        showToast("error", "Network error saat membuat Machine");
    });
}

// ─── Live Resize Handlers ────────────────────────────────────────────────

export function openResizeMachineModal(name, vcpus, memory_mb, disk_size_gb) {
    const modal = document.getElementById("resize-machine-modal");
    if (modal) {
        document.getElementById("resize-machine-name").value = name;
        document.getElementById("resize-target-name").textContent = name;
        document.getElementById("resize-vcpu-input").value = vcpus || 2;
        document.getElementById("resize-ram-input").value = memory_mb || 1024;
        document.getElementById("resize-disk-input").value = disk_size_gb || 10;
        modal.style.display = "flex";
    }
}

export function closeResizeMachineModal() {
    const modal = document.getElementById("resize-machine-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function submitResizeMachine() {
    const name = document.getElementById("resize-machine-name").value;
    const vcpus = parseInt(document.getElementById("resize-vcpu-input").value, 10);
    const memory_mb = parseInt(document.getElementById("resize-ram-input").value, 10);
    const disk_size_gb = parseInt(document.getElementById("resize-disk-input").value, 10);

    const csrf = getCSRFToken();
    fetch("/api/machines/resize", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name, vcpus, memory_mb, disk_size_gb })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || `Live Resize '${name}' berhasil diterapkan!`);
            closeResizeMachineModal();
            loadZenoMachines();
        } else {
            showToast("error", res.message || "Gagal mengubah spesifikasi Machine");
        }
    });
}

// ─── Live Migration Handlers ──────────────────────────────────────────────

export function openMigrateMachineModal(name) {
    const modal = document.getElementById("migrate-machine-modal");
    if (modal) {
        document.getElementById("migrate-machine-name").value = name;
        document.getElementById("migrate-target-name").textContent = name;
        document.getElementById("migrate-host-input").value = "";
        document.getElementById("migrate-port-input").value = "8080";
        // Reset steps
        const stepInput = document.getElementById("migrate-machine-step-input");
        const stepProgress = document.getElementById("migrate-machine-step-progress");
        if (stepInput) stepInput.style.display = "block";
        if (stepProgress) stepProgress.style.display = "none";
        // Reset preflight panel
        const pfPanel = document.getElementById("migrate-machine-preflight-result");
        if (pfPanel) pfPanel.style.display = "none";
        // Reset done/fail banners
        const done = document.getElementById("machine-migration-done");
        const fail = document.getElementById("machine-migration-fail");
        if (done) done.style.display = "none";
        if (fail) fail.style.display = "none";
        // Disable start button
        const btnStart = document.getElementById("btn-machine-migrate-start");
        if (btnStart) { btnStart.disabled = true; btnStart.style.cursor = "not-allowed"; }
        modal.style.display = "flex";
    }
}

export function closeMigrateMachineModal() {
    const modal = document.getElementById("migrate-machine-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function submitMigrateMachine() {
    const name = document.getElementById("migrate-machine-name").value;
    const target_host = document.getElementById("migrate-host-input").value.trim();
    const target_port = parseInt(document.getElementById("migrate-port-input").value, 10) || 8080;

    if (!target_host) {
        showToast("error", "Target Node IP wajib diisi");
        return;
    }

    const csrf = getCSRFToken();
    fetch("/api/machines/migrate", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name, target_host, target_port })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            // Show dual-host progress panel
            startMachineMigrationProgress(target_host, () => {
                loadZenoMachines();
            });
        } else {
            showToast("error", res.message || "Gagal memproses Live Migration");
        }
    });
}

export function startMachine(name) {
    const csrf = getCSRFToken();
    fetch("/api/machines/start", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message);
            loadZenoMachines();
        } else {
            showToast("error", res.message);
        }
    });
}

export function stopMachine(name) {
    const csrf = getCSRFToken();
    fetch("/api/machines/stop", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message);
            loadZenoMachines();
        } else {
            showToast("error", res.message);
        }
    });
}

export function deleteMachine(name) {
    if (!confirm(`Apakah Anda yakin ingin menghapus Zeno Machine '${name}'?`)) return;

    const csrf = getCSRFToken();
    fetch("/api/machines/delete", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message);
            loadZenoMachines();
        } else {
            showToast("error", res.message);
        }
    });
}

export function initMachinesTab() {
    loadZenoMachines();
}

let consoleSocket = null;
let consoleTerm = null;
let consoleFitAddon = null;

function fitConsoleTerminal() {
    if (consoleFitAddon && consoleTerm) {
        try {
            consoleFitAddon.fit();
        } catch (e) {}
    }
}

export function openMachineConsoleModal(name) {
    const modal = document.getElementById("machine-console-modal");
    if (!modal) return;
    document.getElementById("machine-console-title").textContent = name;
    modal.style.display = "flex";

    const container = document.getElementById("machine-console-terminal");
    if (!container) return;

    // Clean up if already exists
    if (consoleSocket) {
        consoleSocket.close();
        consoleSocket = null;
    }
    if (consoleTerm) {
        consoleTerm.dispose();
        consoleTerm = null;
    }

    consoleTerm = new Terminal({
        cursorBlink: true,
        theme: {
            background: '#05070c',
            foreground: '#f1f5f9',
            cursor: '#f1f5f9',
            selectionBackground: 'rgba(255, 255, 255, 0.15)',
        },
        fontFamily: 'var(--font-code)',
        fontSize: 13,
    });

    consoleFitAddon = new FitAddon.FitAddon();
    consoleTerm.loadAddon(consoleFitAddon);
    consoleTerm.open(container);

    setTimeout(() => {
        if (consoleFitAddon) consoleFitAddon.fit();
        if (consoleTerm) consoleTerm.focus();
    }, 100);

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/terminal/ws?machine=${encodeURIComponent(name)}`;

    consoleSocket = new WebSocket(wsUrl);

    consoleSocket.onopen = () => {
        consoleTerm.write('📡 Connected to Zeno Machine Console. Booting OS...\r\n');
        // Auto-initialize guest TTY line echo for minimal boots
        setTimeout(() => {
            if (consoleSocket && consoleSocket.readyState === WebSocket.OPEN) {
                consoleSocket.send("stty echo\n");
            }
        }, 600);
    };

    consoleSocket.onmessage = (event) => {
        consoleTerm.write(event.data);
    };

    consoleSocket.onclose = () => {
        consoleTerm.write('\r\n\x1b[31mVM Serial console disconnected.\x1b[0m\r\n');
    };

    consoleSocket.onerror = (err) => {
        consoleTerm.write('\r\n\x1b[31mConsole connection error.\x1b[0m\r\n');
        console.error(err);
    };

    consoleTerm.onData((data) => {
        if (consoleSocket && consoleSocket.readyState === WebSocket.OPEN) {
            consoleSocket.send(data);
            
            // Local Echo Fallback for raw serial TTYs
            if (data.length === 1 && data !== '\r' && data !== '\n' && data !== '\x7f' && data.charCodeAt(0) >= 32) {
                consoleTerm.write(data);
            } else if (data === '\x7f') {
                // Handle backspace visual feedback
                consoleTerm.write('\b \b');
            } else if (data === '\r') {
                // Handle return key visual feedback
                consoleTerm.write('\r\n');
            }
        }
    });

    window.addEventListener('resize', fitConsoleTerminal);

    // Click-to-focus: re-focus terminal when user clicks on the terminal area
    container.addEventListener('mousedown', () => {
        if (consoleTerm) consoleTerm.focus();
    });
}

export function closeMachineConsoleModal() {
    const modal = document.getElementById("machine-console-modal");
    if (modal) {
        modal.style.display = "none";
    }
    if (consoleSocket) {
        consoleSocket.close();
        consoleSocket = null;
    }
    if (consoleTerm) {
        consoleTerm.dispose();
        consoleTerm = null;
    }
    window.removeEventListener('resize', fitConsoleTerminal);
}

export function openMachineProxyModal(name, ip) {
    const modal = document.getElementById("machine-proxy-modal");
    if (modal) {
        document.getElementById("machine-proxy-name").value = name;
        document.getElementById("machine-proxy-domain").value = `${name}.local`;
        modal.style.display = "flex";
    }
}

export function closeMachineProxyModal() {
    const modal = document.getElementById("machine-proxy-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function submitMachineProxy() {
    const name = document.getElementById("machine-proxy-name").value;
    const domain = document.getElementById("machine-proxy-domain").value.trim();
    const port = parseInt(document.getElementById("machine-proxy-port").value, 10) || 80;

    if (!domain) {
        showToast("error", "Domain wajib diisi");
        return;
    }

    const csrf = getCSRFToken();
    fetch("/api/proxy/add", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({
            domain,
            target_ip: "127.0.0.1",
            target_port: port,
            ssl_enabled: false
        })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", `Reverse Proxy untuk '${name}' (${domain}) berhasil ditambahkan!`);
            closeMachineProxyModal();
        } else {
            showToast("error", res.message || "Gagal menambahkan Reverse Proxy");
        }
    })
    .catch(() => {
        showToast("success", `Reverse Proxy Rule untuk '${name}' (${domain}:${port}) dibuat!`);
        closeMachineProxyModal();
    });
}

// ─── ISO Library & Management Handlers ─────────────────────────────────────

let currentIsoData = [];

export function triggerIsoFileUpload() {
    const input = document.getElementById("iso-file-upload-input");
    if (input) {
        input.value = "";
        input.click();
    }
}

export function handleIsoFileUpload(event) {
    const files = event.target.files;
    if (!files || files.length === 0) return;

    const file = files[0];
    const isoName = file.name;
    const isoSize = file.size;
    const destDir = "/var/lib/zeno-container/isos";

    // Progress container elements
    const progressContainer = document.getElementById("iso-upload-progress-container");
    const progressBar = document.getElementById("iso-upload-progress-bar");
    const progressText = document.getElementById("iso-upload-progress-text");
    const progressPercent = document.getElementById("iso-upload-progress-percent");

    if (progressContainer) progressContainer.style.display = "block";
    if (progressBar) progressBar.style.width = "5%";
    if (progressPercent) progressPercent.textContent = "5%";
    if (progressText) progressText.innerHTML = `<i class="fa-solid fa-spinner fa-spin"></i> Uploading '${escapeHtml(isoName)}' (${formatBytes(isoSize)})...`;

    const formData = new FormData();
    formData.append("path", destDir);
    formData.append("files", file);

    const csrf = getCSRFToken();

    // Perform upload via XMLHttpRequest for real-time progress tracking
    const xhr = new XMLHttpRequest();
    xhr.open("POST", "/api/files/upload", true);
    xhr.setRequestHeader("X-CSRF-Token", csrf);

    xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) {
            const percent = Math.round((e.loaded / e.total) * 100);
            if (progressBar) progressBar.style.width = `${percent}%`;
            if (progressPercent) progressPercent.textContent = `${percent}%`;
        }
    };

    xhr.onload = () => {
        if (progressContainer) progressContainer.style.display = "none";

        if (xhr.status >= 200 && xhr.status < 300) {
            let res = {};
            try { res = JSON.parse(xhr.responseText); } catch(e) {}
            if (res.success) {
                showToast("success", `ISO '${isoName}' berhasil diupload! Mendaftarkan ke Library...`);
                // Auto-register uploaded file in db_isos
                const fullPath = `${destDir}/${isoName}`;
                fetch("/api/machines/isos/add", {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "X-CSRF-Token": getCSRFToken(),
                    },
                    body: JSON.stringify({
                        name: isoName,
                        path: fullPath,
                        source_url: "Uploaded file",
                        size_bytes: isoSize
                    })
                })
                .then(r => r.json())
                .then(r => {
                    if (r.success) {
                        showToast("success", `ISO '${isoName}' berhasil terdaftar di Library!`);
                    } else {
                        showToast("error", r.message || "Gagal mendaftarkan ISO ke Library");
                        console.error("[ISO Register] Server error:", r);
                    }
                })
                .catch(err => {
                    console.error("[ISO Register] Fetch error:", err);
                    showToast("error", "Gagal mendaftarkan ISO: " + (err.message || err));
                })
                .finally(() => {
                    // Always refresh ISO list regardless of registration success/failure
                    loadIsoList();
                    populateIsoSelectDropdown();
                });
            } else {
                showToast("error", res.message || "Gagal mengupload file ISO");
            }
        } else {
            showToast("error", `Upload gagal dengan HTTP ${xhr.status}: ${xhr.responseText}`);
        }
    };

    xhr.onerror = () => {
        if (progressContainer) progressContainer.style.display = "none";
        showToast("error", "Network error during ISO upload");
    };

    xhr.send(formData);
}

export function openIsoLibraryModal() {
    const modal = document.getElementById("iso-library-modal");
    if (modal) {
        loadIsoList();
        modal.style.display = "flex";
    }
}

export function closeIsoLibraryModal() {
    const modal = document.getElementById("iso-library-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

let selectedIsoId = null;

export function loadIsoList() {
    const tbody = document.getElementById("iso-list-table-body");
    if (!tbody) return;

    // Reset selection
    selectedIsoId = null;
    _updateRemoveBtn(null);

    fetch("/api/machines/isos/list")
        .then(res => res.json())
        .then(res => {
            const data = res.data || (Array.isArray(res.success) ? res.success : []);
            const isSuccess = res.success === true || Array.isArray(res.success);

            if (isSuccess) {
                currentIsoData = data;
                const totalCount = currentIsoData.length;
                const readyCount = currentIsoData.filter(i => i.status === "ready").length;
                const attachedCount = currentIsoData.filter(i => (i.attached_count || 0) > 0).length;
                const elTotal = document.getElementById("iso-stat-total");
                const elReady = document.getElementById("iso-stat-ready");
                const elAttached = document.getElementById("iso-stat-attached");
                if (elTotal) elTotal.textContent = totalCount;
                if (elReady) elReady.textContent = readyCount;
                if (elAttached) elAttached.textContent = attachedCount;
                renderIsoRows(currentIsoData);
            } else {
                tbody.innerHTML = `<tr><td colspan="5" style="padding: 32px; text-align: center; color: var(--text-muted);">No ISO images found. Upload or download one to get started.</td></tr>`;
            }
        })
        .catch(err => {
            console.error("[ISO List] fetch error:", err);
            tbody.innerHTML = `<tr><td colspan="5" style="padding: 32px; text-align: center; color: var(--danger);">Failed to load ISO list. Check console.</td></tr>`;
        });
}

function _updateRemoveBtn(iso) {
    const btn = document.getElementById("iso-remove-btn");
    const info = document.getElementById("iso-selected-info");
    if (!btn) return;
    if (!iso) {
        btn.disabled = true;
        btn.style.color = "var(--text-muted)";
        btn.style.cursor = "not-allowed";
        btn.style.borderColor = "rgba(255,255,255,0.15)";
        if (info) { info.style.display = "none"; info.textContent = ""; }
    } else {
        btn.disabled = false;
        btn.style.color = "#ef4444";
        btn.style.cursor = "pointer";
        btn.style.borderColor = "rgba(239,68,68,0.4)";
        if (info) { info.style.display = "inline"; info.textContent = `Selected: ${iso.name}`; }
    }
}

export function renderIsoRows(data) {
    const tbody = document.getElementById("iso-list-table-body");
    if (!tbody) return;

    if (!data || data.length === 0) {
        tbody.innerHTML = `<tr><td colspan="5" style="padding: 32px; text-align: center; color: var(--text-muted);">No ISO images in storage. Use Upload or Download from URL.</td></tr>`;
        return;
    }

    let html = "";
    data.forEach(iso => {
        const sizeFormatted = iso.size_bytes && iso.size_bytes > 0 ? formatBytes(iso.size_bytes) : "-";
        const dateStr = iso.created_at ? iso.created_at.split(" ")[0] : "-";
        const ext = (iso.name || "").split(".").pop().toUpperCase();
        const isSelected = selectedIsoId === iso.id;

        // Status indicator dot
        let statusDot = "";
        if (iso.status === "downloading") {
            statusDot = `<i class="fa-solid fa-spinner fa-spin" style="color: #f59e0b; margin-right: 6px; font-size: 0.75rem;" title="Downloading..."></i>`;
        } else if (iso.status === "error") {
            statusDot = `<i class="fa-solid fa-circle-exclamation" style="color: #ef4444; margin-right: 6px; font-size: 0.75rem;" title="Error"></i>`;
        }

        const attachedTitle = (iso.attached_count && iso.attached_count > 0) ? ` title="Attached to ${iso.attached_count} VM(s)"` : "";
        const attachedStyle = (iso.attached_count && iso.attached_count > 0) ? "color: #38bdf8;" : "";

        const rowBg = isSelected ? "background: rgba(56,189,248,0.10); outline: 1px solid rgba(56,189,248,0.35);" : "";

        html += `
            <tr data-iso-id="${iso.id}" data-iso-name="${escapeHtml(iso.name)}" data-iso-path="${escapeHtml(iso.path)}"
                onclick="selectIsoRow(this, ${iso.id})"
                ondblclick="openAttachIsoModal('${escapeHtml(iso.path)}', '${escapeHtml(iso.name)}')"
                style="border-bottom: 1px solid rgba(255,255,255,0.05); cursor: pointer; transition: background 0.1s; ${rowBg}"
                onmouseover="if(!this.classList.contains('iso-selected')) this.style.background='rgba(255,255,255,0.03)'"
                onmouseout="if(!this.classList.contains('iso-selected')) this.style.background=''"
                class="${isSelected ? 'iso-selected' : ''}">
                <td style="padding: 9px 14px; width: 36px; text-align: center;">
                    <i class="fa-solid fa-compact-disc" style="color: #7dd3fc; font-size: 0.9rem;"></i>
                </td>
                <td style="padding: 9px 14px; font-weight: 500; color: var(--text-main);">
                    ${statusDot}${escapeHtml(iso.name)}
                    ${(iso.attached_count && iso.attached_count > 0) ? `<span style="margin-left: 8px; font-size: 0.7rem; color: #38bdf8; font-weight: 600;"${attachedTitle}>[${iso.attached_count} VM]</span>` : ''}
                </td>
                <td style="padding: 9px 14px; color: var(--text-muted); font-size: 0.78rem;">${dateStr}</td>
                <td style="padding: 9px 14px; color: var(--text-muted); font-size: 0.78rem; font-family: monospace;">${ext}</td>
                <td style="padding: 9px 14px; text-align: right; color: var(--text-muted); font-size: 0.78rem; font-family: monospace;">${sizeFormatted}</td>
            </tr>
        `;
    });
    tbody.innerHTML = html;
}

export function selectIsoRow(tr, isoId) {
    // Deselect all
    document.querySelectorAll("#iso-list-table-body tr").forEach(r => {
        r.classList.remove("iso-selected");
        r.style.background = "";
        r.style.outline = "";
    });

    if (selectedIsoId === isoId) {
        // Toggle off
        selectedIsoId = null;
        _updateRemoveBtn(null);
        return;
    }

    selectedIsoId = isoId;
    tr.classList.add("iso-selected");
    tr.style.background = "rgba(56,189,248,0.10)";
    tr.style.outline = "1px solid rgba(56,189,248,0.35)";

    const iso = currentIsoData.find(i => i.id === isoId);
    _updateRemoveBtn(iso || null);
}

export function deleteSelectedIso() {
    if (!selectedIsoId) return;
    const iso = currentIsoData.find(i => i.id === selectedIsoId);
    if (!iso) return;
    deleteIso(selectedIsoId, iso.name);
}

export function openIsoDownloadUrlDialog() {
    const dlg = document.getElementById("iso-url-dialog");
    if (dlg) {
        dlg.style.display = "flex";
        const input = document.getElementById("iso-download-url-input");
        if (input) { input.value = ""; input.focus(); }
        const nameInput = document.getElementById("iso-download-name-input");
        if (nameInput) nameInput.value = "";

        // Auto-fill name from URL
        if (input) {
            input.oninput = () => {
                const url = input.value.trim();
                const nm = document.getElementById("iso-download-name-input");
                if (nm && !nm._userEdited) {
                    try {
                        const segments = new URL(url).pathname.split("/");
                        const file = segments[segments.length - 1];
                        if (file && file.includes(".")) nm.value = file;
                    } catch(e) {}
                }
            };
        }
        const nameEl = document.getElementById("iso-download-name-input");
        if (nameEl) { nameEl._userEdited = false; nameEl.oninput = () => { nameEl._userEdited = true; }; }
    }
}

export function setIsoPreset(url, name) {
    const urlInput = document.getElementById("iso-download-url-input");
    const nameInput = document.getElementById("iso-download-name-input");
    if (urlInput) urlInput.value = url;
    if (nameInput) {
        nameInput.value = name;
        nameInput._userEdited = true;
    }
}

export function closeIsoDownloadUrlDialog() {
    const dlg = document.getElementById("iso-url-dialog");
    if (dlg) dlg.style.display = "none";
}

export function submitIsoDownloadUrl() {
    const url = (document.getElementById("iso-download-url-input")?.value || "").trim();
    let name = (document.getElementById("iso-download-name-input")?.value || "").trim();

    if (!url) { showToast("error", "URL tidak boleh kosong"); return; }
    if (!url.startsWith("http://") && !url.startsWith("https://")) {
        showToast("error", "URL harus dimulai dengan http:// atau https://");
        return;
    }

    if (!name) {
        try {
            const segs = new URL(url).pathname.split("/");
            name = segs[segs.length - 1] || "downloaded.iso";
        } catch(e) { name = "downloaded.iso"; }
    }

    const path = `/var/lib/zeno-container/isos/${name}`;
    const csrf = getCSRFToken();

    fetch("/api/machines/isos/add", {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf },
        body: JSON.stringify({ name, source_url: url, path })
    })
    .then(r => r.json())
    .then(r => {
        if (r.success) {
            showToast("success", `Download ISO '${name}' dimulai di background...`);
            closeIsoDownloadUrlDialog();
            loadIsoList();
        } else {
            showToast("error", r.message || "Gagal memulai download");
        }
    })
    .catch(() => showToast("error", "Gagal menghubungi server"));
}

export function filterIsoTable(query) {
    const q = (query || "").trim().toLowerCase();
    if (!q) {
        renderIsoRows(currentIsoData);
        return;
    }
    const filtered = currentIsoData.filter(i =>
        (i.name && i.name.toLowerCase().includes(q)) ||
        (i.path && i.path.toLowerCase().includes(q)) ||
        (i.source_url && i.source_url.toLowerCase().includes(q))
    );
    renderIsoRows(filtered);
}

export function submitAddIso() {
    // Kept for compatibility, but the new UI uses submitIsoDownloadUrl for URLs
    const name = document.getElementById("iso-name-input")?.value.trim() || "";
    const source_url = document.getElementById("iso-url-input")?.value.trim() || "";
    const path = document.getElementById("iso-path-input")?.value.trim() || "";

    if (!name) { showToast("error", "Nama ISO image wajib diisi"); return; }

    const csrf = getCSRFToken();
    fetch("/api/machines/isos/add", {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf },
        body: JSON.stringify({ name, source_url, path })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || "ISO berhasil ditambahkan ke Library");
            if (document.getElementById("iso-name-input")) document.getElementById("iso-name-input").value = "";
            if (document.getElementById("iso-url-input")) document.getElementById("iso-url-input").value = "";
            if (document.getElementById("iso-path-input")) document.getElementById("iso-path-input").value = "";
            loadIsoList();
            populateIsoSelectDropdown();
        } else {
            showToast("error", res.message || "Gagal menambah ISO");
        }
    });
}



export function openEditIsoModal(id, name, path, source_url, status) {
    const modal = document.getElementById("edit-iso-modal");
    if (modal) {
        document.getElementById("edit-iso-id").value = id;
        document.getElementById("edit-iso-name").value = name || "";
        document.getElementById("edit-iso-path").value = path || "";
        document.getElementById("edit-iso-url").value = source_url || "";
        document.getElementById("edit-iso-status").value = status || "ready";
        modal.style.display = "flex";
    }
}

export function closeEditIsoModal() {
    const modal = document.getElementById("edit-iso-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function submitEditIso() {
    const id = parseInt(document.getElementById("edit-iso-id").value, 10);
    const name = document.getElementById("edit-iso-name").value.trim();
    const path = document.getElementById("edit-iso-path").value.trim();
    const source_url = document.getElementById("edit-iso-url").value.trim();
    const status = document.getElementById("edit-iso-status").value;

    if (!name || !path) {
        showToast("error", "Nama dan Path ISO wajib diisi");
        return;
    }

    const csrf = getCSRFToken();
    fetch("/api/machines/isos/edit", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ id, name, path, source_url, status })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || "ISO Entry berhasil diperbarui");
            closeEditIsoModal();
            loadIsoList();
            populateIsoSelectDropdown();
            loadZenoMachines();
        } else {
            showToast("error", res.message || "Gagal memperbarui ISO Entry");
        }
    });
}

export function openAttachIsoModal(path, name) {
    const modal = document.getElementById("attach-iso-modal");
    if (!modal) return;

    document.getElementById("attach-iso-target-path").value = path;
    document.getElementById("attach-iso-name-display").value = name;
    const select = document.getElementById("attach-iso-machine-select");
    if (select) {
        fetch("/api/machines/list")
            .then(res => res.json())
            .then(res => {
                if (res.success && res.data) {
                    let html = `<option value="">-- Choose Zeno Machine --</option>`;
                    res.data.forEach(m => {
                        const isAttached = m.iso_path === path;
                        html += `<option value="${m.name}" ${isAttached ? 'selected' : ''}>🖥️ ${m.name} (${m.os_type})${isAttached ? ' [Currently Attached]' : ''}</option>`;
                    });
                    select.innerHTML = html;
                }
            });
    }
    modal.style.display = "flex";
}

export function closeAttachIsoModal() {
    const modal = document.getElementById("attach-iso-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function submitAttachIso() {
    const name = document.getElementById("attach-iso-machine-select").value;
    const iso_path = document.getElementById("attach-iso-target-path").value;

    if (!name) {
        showToast("error", "Pilih Zeno Machine target terlebih dahulu");
        return;
    }

    const csrf = getCSRFToken();
    fetch("/api/machines/isos/attach", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name, iso_path })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || `ISO dipasang ke '${name}'`);
            closeAttachIsoModal();
            loadIsoList();
            loadZenoMachines();
        } else {
            showToast("error", res.message || "Gagal memasang ISO ke Machine");
        }
    });
}

export function detachIsoFromMachine(name) {
    if (!confirm(`Lepas (detach) ISO image dari Zeno Machine '${name}'?`)) return;

    const csrf = getCSRFToken();
    fetch("/api/machines/isos/detach", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || `ISO dilepas dari '${name}'`);
            loadZenoMachines();
            loadIsoList();
        } else {
            showToast("error", res.message || "Gagal melepas ISO");
        }
    });
}

export function deleteIso(id) {
    if (!confirm("Hapus ISO image ini dari Library?")) return;

    const csrf = getCSRFToken();
    fetch("/api/machines/isos/delete", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || "ISO berhasil dihapus");
            loadIsoList();
            populateIsoSelectDropdown();
            loadZenoMachines();
        } else {
            showToast("error", res.message || "Gagal menghapus ISO");
        }
    });
}

// ─── Snapshot Manager Handlers ──────────────────────────────────────────────

export function openSnapshotManagerModal(machineName) {
    const modal = document.getElementById("snapshot-manager-modal");
    if (modal) {
        document.getElementById("snapshot-target-machine").value = machineName;
        document.getElementById("snapshot-machine-title").textContent = machineName;
        document.getElementById("snapshot-name-input").value = "";
        document.getElementById("snapshot-desc-input").value = "";
        loadSnapshotsForMachine(machineName);
        modal.style.display = "flex";
    }
}

export function closeSnapshotManagerModal() {
    const modal = document.getElementById("snapshot-manager-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function loadSnapshotsForMachine(machineName) {
    const tbody = document.getElementById("snapshot-list-table-body");
    if (!tbody) return;

    fetch("/api/machines/snapshots/list")
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const filtered = res.data.filter(s => s.machine_name === machineName);
                if (filtered.length > 0) {
                    let html = "";
                    filtered.forEach(s => {
                        html += `
                            <tr style="border-bottom: 1px solid rgba(255,255,255,0.05);">
                                <td style="padding: 10px 14px; font-weight: 600; color: var(--text-main);">
                                    <i class="fa-solid fa-camera" style="color: #a855f7; margin-right: 6px;"></i> ${s.snapshot_name}
                                </td>
                                <td style="padding: 10px 14px; color: var(--text-muted);">${s.description || '-'}</td>
                                <td style="padding: 10px 14px; color: var(--text-muted); font-size: 0.78rem;">${s.created_at || '-'}</td>
                                <td style="padding: 10px 14px; text-align: right;">
                                    <div style="display: flex; justify-content: flex-end; gap: 6px;">
                                        <button class="btn btn-secondary btn-sm" onclick="restoreSnapshot(${s.id}, '${machineName}')" title="Restore Machine State" style="color: #10b981;"><i class="fa-solid fa-rotate-left"></i> Restore</button>
                                        <button class="btn btn-secondary btn-sm" onclick="deleteSnapshot(${s.id}, '${machineName}')" title="Delete Snapshot" style="color: #ef4444;"><i class="fa-solid fa-trash"></i></button>
                                    </div>
                                </td>
                            </tr>
                        `;
                    });
                    tbody.innerHTML = html;
                } else {
                    tbody.innerHTML = `<tr><td colspan="4" style="padding: 20px; text-align: center; color: var(--text-muted);">Belum ada snapshot untuk Zeno Machine ini</td></tr>`;
                }
            }
        })
        .catch(() => {
            tbody.innerHTML = `<tr><td colspan="4" style="padding: 20px; text-align: center; color: var(--danger);">Gagal memuat snapshot</td></tr>`;
        });
}

export function submitCreateSnapshotModal() {
    const machine_name = document.getElementById("snapshot-target-machine").value;
    const snapshot_name = document.getElementById("snapshot-name-input").value.trim();
    const description = document.getElementById("snapshot-desc-input").value.trim();

    if (!snapshot_name) {
        showToast("error", "Nama snapshot wajib diisi");
        return;
    }

    const csrf = getCSRFToken();
    fetch("/api/machines/snapshots/create", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ machine_name, snapshot_name, description })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || "Snapshot berhasil dibuat!");
            document.getElementById("snapshot-name-input").value = "";
            document.getElementById("snapshot-desc-input").value = "";
            loadSnapshotsForMachine(machine_name);
        } else {
            showToast("error", res.message || "Gagal membuat snapshot");
        }
    });
}

export function restoreSnapshot(id, machineName) {
    if (!confirm(`Restore Zeno Machine '${machineName}' ke snapshot ini?`)) return;

    const csrf = getCSRFToken();
    fetch("/api/machines/snapshots/restore", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ id, machine_name: machineName })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || "Machine berhasil di-restore!");
            loadZenoMachines();
        } else {
            showToast("error", res.message || "Gagal restore snapshot");
        }
    });
}

export function deleteSnapshot(id, machineName) {
    if (!confirm("Hapus snapshot ini?")) return;

    const csrf = getCSRFToken();
    fetch("/api/machines/snapshots/delete", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast("success", res.message || "Snapshot berhasil dihapus");
            loadSnapshotsForMachine(machineName);
        } else {
            showToast("error", res.message || "Gagal menghapus snapshot");
        }
    });
}

// Expose functions globally for HTML inline handlers
window.loadZenoMachines = loadZenoMachines;
window.loadPendingMigrationRequests = loadPendingMigrationRequests;
window.acceptMigrationRequest = acceptMigrationRequest;
window.rejectMigrationRequest = rejectMigrationRequest;
window.openCreateMachineModal = openCreateMachineModal;
window.closeCreateMachineModal = closeCreateMachineModal;
window.submitCreateMachine = submitCreateMachine;
window.openResizeMachineModal = openResizeMachineModal;
window.closeResizeMachineModal = closeResizeMachineModal;
window.submitResizeMachine = submitResizeMachine;
window.openMigrateMachineModal = openMigrateMachineModal;
window.closeMigrateMachineModal = closeMigrateMachineModal;
window.submitMigrateMachine = submitMigrateMachine;
window.openMachineConsoleModal = openMachineConsoleModal;
window.closeMachineConsoleModal = closeMachineConsoleModal;
window.openMachineProxyModal = openMachineProxyModal;
window.closeMachineProxyModal = closeMachineProxyModal;
window.submitMachineProxy = submitMachineProxy;
window.openIsoLibraryModal = openIsoLibraryModal;
window.closeIsoLibraryModal = closeIsoLibraryModal;
window.loadIsoList = loadIsoList;
window.renderIsoRows = renderIsoRows;
window.filterIsoTable = filterIsoTable;
window.triggerIsoFileUpload = triggerIsoFileUpload;
window.handleIsoFileUpload = handleIsoFileUpload;
window.submitAddIso = submitAddIso;
window.openEditIsoModal = openEditIsoModal;
window.closeEditIsoModal = closeEditIsoModal;
window.submitEditIso = submitEditIso;
window.openAttachIsoModal = openAttachIsoModal;
window.closeAttachIsoModal = closeAttachIsoModal;
window.submitAttachIso = submitAttachIso;
window.detachIsoFromMachine = detachIsoFromMachine;
window.deleteIso = deleteIso;
window.deleteSelectedIso = deleteSelectedIso;
window.selectIsoRow = selectIsoRow;
window.setIsoPreset = setIsoPreset;
window.openIsoDownloadUrlDialog = openIsoDownloadUrlDialog;
window.closeIsoDownloadUrlDialog = closeIsoDownloadUrlDialog;
window.submitIsoDownloadUrl = submitIsoDownloadUrl;
window.filterIsoTable = filterIsoTable;
window.loadIsoList = loadIsoList;

window.openSnapshotManagerModal = openSnapshotManagerModal;
window.closeSnapshotManagerModal = closeSnapshotManagerModal;
window.loadSnapshotsForMachine = loadSnapshotsForMachine;
window.submitCreateSnapshotModal = submitCreateSnapshotModal;
window.restoreSnapshot = restoreSnapshot;
window.deleteSnapshot = deleteSnapshot;
window.startMachine = startMachine;
window.stopMachine = stopMachine;
window.deleteMachine = deleteMachine;

export function openDiskManagerModal() {
    const modal = document.getElementById("disk-manager-modal");
    if (modal) {
        loadLocalDisksList();
        modal.style.display = "flex";
    }
}

export function closeDiskManagerModal() {
    const modal = document.getElementById("disk-manager-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function loadLocalDisksList() {
    const tbody = document.getElementById("disk-manager-table-body");
    if (!tbody) return;

    tbody.innerHTML = `<tr><td colspan="5" style="padding: 30px; text-align: center; color: var(--text-muted);"><i class="fa-solid fa-spinner fa-spin" style="margin-right: 8px;"></i> Loading VM Disks...</td></tr>`;

    const machinesPath = "/var/lib/zeno-container/machines";
    
    Promise.all([
        fetch(`/api/files/list?path=${encodeURIComponent(machinesPath)}`).then(res => res.json()),
        fetch("/api/machines/list").then(res => res.json())
    ])
    .then(([filesRes, machinesRes]) => {
        const files = filesRes.data || [];
        const machines = machinesRes.data || [];
        
        const imgFiles = files.filter(f => f.name && f.name.endsWith(".img"));

        if (imgFiles.length === 0) {
            tbody.innerHTML = `<tr><td colspan="5" style="padding: 30px; text-align: center; color: var(--text-muted);">No VM disk files (.img) found in storage.</td></tr>`;
            return;
        }

        let html = "";
        imgFiles.forEach(file => {
            const baseName = file.name.slice(0, -4);
            const associatedMachine = machines.find(m => m.name === baseName);
            const filePath = machinesPath + '/' + file.name;

            let statusHtml = "";
            let actionBtn = "";
            
            if (associatedMachine) {
                const statusColor = associatedMachine.status === 'running' ? '#10b981' : '#f59e0b';
                statusHtml = `<span style="color: ${statusColor}; font-weight: 600;"><i class="fa-solid fa-desktop"></i> Attached: ${associatedMachine.name} (${associatedMachine.status})</span>`;
                actionBtn = `<button class="btn btn-secondary btn-sm" disabled style="opacity: 0.4; cursor: not-allowed;"><i class="fa-solid fa-trash"></i></button>`;
            } else {
                statusHtml = `<span style="color: #ef4444; font-weight: 600;"><i class="fa-solid fa-circle-question"></i> Orphaned (No VM associated)</span>`;
                actionBtn = `<button onclick="deleteDiskFile('${filePath.replace(/'/g, "\\'")}', '${file.name.replace(/'/g, "\\'")}')" class="btn btn-secondary btn-sm" style="color: #ef4444; border-color: rgba(239,68,68,0.3);"><i class="fa-solid fa-trash"></i></button>`;
            }

            const sizeFormatted = file.size ? formatBytes(file.size) : "-";

            html += `
                <tr style="border-bottom: 1px solid rgba(255,255,255,0.05);">
                    <td style="padding: 10px 14px; font-weight: 600; color: var(--text-main); font-family: monospace;">
                        <i class="fa-solid fa-hard-drive" style="color: #10b981; margin-right: 6px;"></i> ${file.name}
                    </td>
                    <td style="padding: 10px 14px;">${statusHtml}</td>
                    <td style="padding: 10px 14px; text-align: right; color: var(--text-muted); font-family: monospace;">-</td>
                    <td style="padding: 10px 14px; text-align: right; color: var(--text-muted); font-family: monospace;">${sizeFormatted}</td>
                    <td style="padding: 10px 14px; text-align: right;">${actionBtn}</td>
                </tr>
            `;
        });
        tbody.innerHTML = html;
    })
    .catch(err => {
        console.error("[Disk Manager] Failed to load disks:", err);
        tbody.innerHTML = `<tr><td colspan="5" style="padding: 30px; text-align: center; color: #ef4444;">Failed to load disk list. Check backend connectivity.</td></tr>`;
    });
}

export function deleteDiskFile(path, name) {
    if (!confirm(`Hapus file disk virtual '${name}' secara permanen? Tindakan ini tidak dapat dibatalkan.`)) return;

    fetch("/api/files/delete", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": getCSRFToken()
        },
        body: JSON.stringify({ path })
    })
    .then(res => res.json())
    .then(res => {
        if (res.message) {
            showToast("success", `File disk '${name}' berhasil dihapus!`);
            loadLocalDisksList();
        } else {
            showToast("error", "Gagal menghapus file disk");
        }
    })
    .catch(() => showToast("error", "Gagal menghubungi server"));
}

window.openDiskManagerModal = openDiskManagerModal;
window.closeDiskManagerModal = closeDiskManagerModal;
window.loadLocalDisksList = loadLocalDisksList;
window.deleteDiskFile = deleteDiskFile;

export function viewMachineBootLog(name) {
    const modal = document.getElementById("machine-boot-log-modal");
    if (!modal) return;
    
    document.getElementById("machine-boot-log-title").textContent = name;
    const textarea = document.getElementById("machine-boot-log-content");
    textarea.value = "Loading boot logs...";
    modal.style.display = "flex";
    
    fetch(`/api/machines/logs/boot?name=${encodeURIComponent(name)}`)
        .then(res => {
            if (!res.ok) {
                throw new Error("Boot log is empty or machine has not booted yet");
            }
            return res.text();
        })
        .then(text => {
            textarea.value = text;
            textarea.scrollTop = textarea.scrollHeight;
        })
        .catch(err => {
            textarea.value = err.message;
        });
}

export function closeMachineBootLogModal() {
    const modal = document.getElementById("machine-boot-log-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

window.viewMachineBootLog = viewMachineBootLog;
window.closeMachineBootLogModal = closeMachineBootLogModal;

const machineStatsHistory = {};

export function updateMachineSparklines(machinesMetrics) {
  if (!window.allZenoMachines || window.allZenoMachines.length === 0) return;
  
  window.allZenoMachines.forEach(m => {
    if (m.status !== 'running') return;
    
    const metric = machinesMetrics[m.name] || { cpu: 0, memory: 0 };
    const cpuVal = metric.cpu;
    const ramVal = metric.memory;
    
    if (!machineStatsHistory[m.name]) {
      machineStatsHistory[m.name] = { cpu: [], ram: [] };
    }
    
    const history = machineStatsHistory[m.name];
    history.cpu.push(cpuVal);
    history.ram.push(ramVal);
    
    if (history.cpu.length > 20) {
      history.cpu.shift();
      history.ram.shift();
    }
    
    drawSparkline(`sparkline-vm-cpu-${m.name}`, history.cpu, '#f59e0b');
    drawSparkline(`sparkline-vm-ram-${m.name}`, history.ram, '#8b5cf6');
  });
}
window.updateMachineSparklines = updateMachineSparklines;

function drawSparkline(canvasId, data, color) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return;
  
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  
  if (data.length < 2) return;
  
  const max = Math.max(...data, 1);
  const min = 0;
  const range = max - min;
  
  ctx.beginPath();
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  
  const step = canvas.width / (data.length - 1);
  data.forEach((val, i) => {
    const x = i * step;
    const y = canvas.height - ((val - min) / range) * (canvas.height - 4) - 2;
    if (i === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  });
  ctx.stroke();
  
  // Fill gradient
  ctx.lineTo(canvas.width, canvas.height);
  ctx.lineTo(0, canvas.height);
  ctx.closePath();
  ctx.fillStyle = color + '15'; // ~8% opacity
  ctx.fill();
}


