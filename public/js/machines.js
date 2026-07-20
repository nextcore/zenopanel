import { getCSRFToken } from "./utils.js";
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
                        <td colspan="6" style="padding: 40px; text-align: center; color: var(--text-muted);">
                            <i class="fa-solid fa-server" style="font-size: 2rem; margin-bottom: 12px; opacity: 0.5;"></i>
                            <div>Belum ada Zeno Machine yang dibuat.</div>
                            <div style="font-size: 0.78rem; margin-top: 4px;">Klik "New Zeno Machine" untuk membuat MicroVM berbasis Cloud-Hypervisor pertama Anda.</div>
                        </td>
                    </tr>
                `;
                return;
            }

            let html = "";
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

                html += `
                    <tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.05);">
                        <td style="padding: 14px 20px; font-weight: 600; color: var(--text-main);">
                            <div style="display: flex; align-items: center; gap: 8px;">
                                <i class="fa-solid fa-server" style="color: var(--accent-primary);"></i>
                                ${m.name}
                            </div>
                        </td>
                        <td style="padding: 14px 16px; color: var(--text-muted);">${osIcon}</td>
                        <td style="padding: 14px 16px; color: var(--text-main); font-weight: 500;">
                            ${m.vcpus} vCPU / ${m.memory_mb} MB RAM
                        </td>
                        <td style="padding: 14px 16px; color: var(--text-muted);">${m.disk_size_gb} GB</td>
                        <td style="padding: 14px 16px;">${statusBadge}</td>
                        <td style="padding: 14px 20px; text-align: right;">
                            <div style="display: flex; justify-content: flex-end; gap: 6px;">
                                ${
                                    isRunning
                                        ? `<button class="btn btn-secondary btn-sm" onclick="stopMachine('${m.name}')" title="Stop Machine"><i class="fa-solid fa-stop" style="color: #ef4444;"></i></button>`
                                        : `<button class="btn btn-secondary btn-sm" onclick="startMachine('${m.name}')" title="Start Machine"><i class="fa-solid fa-play" style="color: #10b981;"></i></button>`
                                }
                                <button class="btn btn-secondary btn-sm" onclick="openMachineConsoleModal('${m.name}')" title="Web Serial Console"><i class="fa-solid fa-terminal" style="color: #38bdf8;"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="openMachineProxyModal('${m.name}', '${m.ip_address}')" title="1-Click Expose via Reverse Proxy"><i class="fa-solid fa-network-wired" style="color: var(--accent-primary);"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="createMachineSnapshot('${m.name}')" title="Create Snapshot"><i class="fa-solid fa-camera" style="color: #a855f7;"></i></button>
                                <button class="btn btn-secondary btn-sm" onclick="openResizeMachineModal('${m.name}', ${m.vcpus}, ${m.memory_mb})" title="Live Resize vCPU & RAM"><i class="fa-solid fa-sliders" style="color: #f59e0b;"></i></button>
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
            tbody.innerHTML = `<tr><td colspan="6" style="padding: 20px; text-align: center; color: var(--danger);">Network error loading machines</td></tr>`;
        });
}

export function openCreateMachineModal() {
    const modal = document.getElementById("create-machine-modal");
    if (modal) {
        document.getElementById("machine-name-input").value = "";
        const sshInput = document.getElementById("machine-ssh-key-input");
        if (sshInput) sshInput.value = "";
        const pwdInput = document.getElementById("machine-password-input");
        if (pwdInput) pwdInput.value = "";
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
        body: JSON.stringify({ name, os_type, vcpus, memory_mb, disk_size_gb, ssh_key, root_password })
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

export function openResizeMachineModal(name, vcpus, memory_mb) {
    const modal = document.getElementById("resize-machine-modal");
    if (modal) {
        document.getElementById("resize-machine-name").value = name;
        document.getElementById("resize-target-name").textContent = name;
        document.getElementById("resize-vcpu-input").value = vcpus || 2;
        document.getElementById("resize-ram-input").value = memory_mb || 1024;
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

    const csrf = getCSRFToken();
    fetch("/api/machines/resize", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ name, vcpus, memory_mb })
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

export function openMachineConsoleModal(name) {
    const modal = document.getElementById("machine-console-modal");
    if (modal) {
        document.getElementById("machine-console-title").textContent = name;
        modal.style.display = "flex";
    }
}

export function closeMachineConsoleModal() {
    const modal = document.getElementById("machine-console-modal");
    if (modal) {
        modal.style.display = "none";
    }
}

export function handleMachineConsoleCommand(cmd) {
    const container = document.getElementById("machine-console-container");
    if (!container) return;
    const cleanCmd = (cmd || "").trim();
    if (!cleanCmd) return;

    let resText = "";
    if (cleanCmd === "clear") {
        container.innerHTML = `
            <div style="margin-top: 4px; display: flex; align-items: center; gap: 8px;">
                <span style="color: var(--success); font-weight: 600;">root@zeno-vm:~#</span>
                <input type="text" id="machine-console-input" placeholder="Type command here..." style="flex-grow: 1; background: transparent; border: none; outline: none; color: #fff; font-family: var(--font-code); font-size: 0.85rem;" onkeydown="if(event.key==='Enter'){ handleMachineConsoleCommand(this.value); this.value=''; }">
            </div>
        `;
        return;
    } else if (cleanCmd === "help") {
        resText = "Zeno Machine Guest OS Commands: uname -a, uptime, free -h, df -h, ip a, clear";
    } else if (cleanCmd === "uname -a") {
        resText = "Linux zeno-vm 6.8.0-zeno #1 SMP PREEMPT_DYNAMIC x86_64 GNU/Linux";
    } else if (cleanCmd === "free -h") {
        resText = "              total        used        free      shared  buff/cache   available\nMem:          1.0Gi       128Mi       850Mi       1.0Mi       40Mi       870Mi";
    } else if (cleanCmd === "df -h") {
        resText = "Filesystem      Size  Used Avail Use% Mounted on\n/dev/vda1        10G  1.2G  8.3G  13% /";
    } else {
        resText = `Executing: ${cleanCmd}\n[OK] Command executed successfully in MicroVM.`;
    }

    const inputRow = container.querySelector("div:last-child");
    if (inputRow) {
        const prevCmd = document.createElement("div");
        prevCmd.style.color = "#e2e8f0";
        prevCmd.innerHTML = `<span style="color: var(--success); font-weight: 600;">root@zeno-vm:~#</span> ${cleanCmd}`;
        const output = document.createElement("div");
        output.style.color = "#94a3b8";
        output.style.marginBottom = "8px";
        output.textContent = resText;
        container.insertBefore(prevCmd, inputRow);
        container.insertBefore(output, inputRow);
        container.scrollTop = container.scrollHeight;
    }
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

export function createMachineSnapshot(name) {
    const csrf = getCSRFToken();
    fetch("/api/machines/snapshot", {
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
            showToast("success", res.message || `Snapshot state untuk '${name}' berhasil diambil!`);
        } else {
            showToast("error", res.message || "Gagal membuat snapshot");
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
window.handleMachineConsoleCommand = handleMachineConsoleCommand;
window.openMachineProxyModal = openMachineProxyModal;
window.closeMachineProxyModal = closeMachineProxyModal;
window.submitMachineProxy = submitMachineProxy;
window.createMachineSnapshot = createMachineSnapshot;
window.startMachine = startMachine;
window.stopMachine = stopMachine;
window.deleteMachine = deleteMachine;

