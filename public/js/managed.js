import { getCSRFToken, escapeHtml } from './utils.js';
import { showToast } from './toast.js';

export let managedPollingInterval = null;
export let logPollingInterval = null;
export let activeLogProcessId = null;
export const managedState = {
    allManagedProcesses: []
};
export let dpCurrentPath = '.';

export function loadManagedProcesses() {
    fetch('/api/managed/list')
        .then(res => res.json())
        .then(res => {
            if (res.data) {
                managedState.allManagedProcesses = res.data;
                renderManagedProcesses(res.data);
            }
        })
        .catch(err => {
            console.error('Failed to load managed processes:', err);
        });
}

export function renderManagedProcesses(processes) {
    const tbody = document.getElementById('managed-table-body');
    if (!tbody) return;
    tbody.innerHTML = '';
    
    if (processes.length === 0) {
        tbody.innerHTML = `
            <tr>
                <td colspan="9" style="text-align:center; padding:30px; color:var(--text-muted);">
                    <i class="fa-solid fa-gears" style="font-size:2rem; margin-bottom:10px; display:block; opacity:0.3;"></i>
                    No managed processes registered yet.
                </td>
            </tr>
        `;
        return;
    }

    processes.forEach(proc => {
        let statusBadge = '<span class="status-badge stopped" style="background:rgba(255,255,255,0.05); color:var(--text-muted); border:1px solid var(--card-border); padding:3px 8px; border-radius:4px; font-size:0.75rem;">Stopped</span>';
        if (proc.status === 'running') {
            statusBadge = '<span class="status-badge running" style="background:rgba(16,185,129,0.1); color:var(--success); border:1px solid rgba(16,185,129,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem; display:inline-flex; align-items:center; gap:6px;"><span class="pulse-indicator"></span> Running</span>';
        } else if (proc.status === 'starting') {
            statusBadge = '<span class="status-badge starting" style="background:rgba(245,158,11,0.1); color:#f59e0b; border:1px solid rgba(245,158,11,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem;">Starting</span>';
        } else if (proc.status === 'failed') {
            statusBadge = '<span class="status-badge failed" style="background:rgba(239,68,68,0.1); color:#ef4444; border:1px solid rgba(239,68,68,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem;">Failed</span>';
        }

        const tr = document.createElement('tr');
        
        const cpuVal = proc.status === 'running' && typeof proc.cpu_usage === 'number' ? proc.cpu_usage.toFixed(1) + '%' : '-';
        const memVal = proc.status === 'running' && typeof proc.memory_usage === 'number' ? proc.memory_usage.toFixed(1) + ' MB' : '-';
        const portVal = proc.port ? proc.port : '-';

        tr.innerHTML = `
            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(proc.name)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; max-width:250px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;" title="${escapeHtml(proc.command)}">${escapeHtml(proc.command)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:#10b981; font-weight:600;">${cpuVal}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:#6366f1; font-weight:600;">${memVal}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--warning); font-weight:600;">${portVal}</td>
            <td style="font-size:0.85rem; color:var(--text-muted);">${escapeHtml(proc.cwd)}</td>
            <td>${statusBadge}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${proc.pid ? proc.pid : '-'}</td>
            <td style="text-align:right; overflow:visible;">
                <div class="action-dropdown" id="dropdown-${proc.id}">
                    <button class="action-dropdown-btn" onclick="toggleProcessDropdown(event, '${proc.id}')">
                        <i class="fa-solid fa-ellipsis-vertical"></i> Manage
                    </button>
                    <div class="action-dropdown-menu" id="menu-${proc.id}">
                        ${proc.status === 'running' || proc.status === 'starting' ? `
                            <button class="action-dropdown-item" onclick="stopProcess('${proc.id}')">
                                <i class="fa-solid fa-stop" style="color:#ef4444;"></i> Stop
                            </button>
                        ` : `
                            <button class="action-dropdown-item" onclick="startProcess('${proc.id}')">
                                <i class="fa-solid fa-play" style="color:var(--success);"></i> Start
                            </button>
                        `}
                        <button class="action-dropdown-item" onclick="restartProcess('${proc.id}')">
                            <i class="fa-solid fa-rotate-right" style="color:var(--accent-primary);"></i> Restart
                        </button>
                        <button class="action-dropdown-item" onclick="openEditProcessModal('${proc.id}')">
                            <i class="fa-solid fa-pen-to-square" style="color:var(--warning);"></i> Edit
                        </button>
                        <button class="action-dropdown-item" onclick="viewProcessLogs('${proc.id}', '${escapeHtml(proc.name)}')">
                            <i class="fa-solid fa-terminal" style="color:var(--text-main);"></i> Logs
                        </button>
                        <hr style="border:none; border-top:1px solid var(--card-border); margin:4px 0;">
                        <button class="action-dropdown-item danger" onclick="deleteProcess('${proc.id}')">
                            <i class="fa-solid fa-trash-can"></i> Delete
                        </button>
                    </div>
                </div>
            </td>
        `;
        tbody.appendChild(tr);
    });
}

export function startManagedPolling() {
    if (managedPollingInterval) clearInterval(managedPollingInterval);
    managedPollingInterval = setInterval(loadManagedProcesses, 3000);
}

export function stopManagedPolling() {
    if (managedPollingInterval) {
        clearInterval(managedPollingInterval);
        managedPollingInterval = null;
    }
}

export function startProcess(id) {
    fetch('/api/managed/start', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id: id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            loadManagedProcesses();
        } else {
            showToast('error', res.message || 'Gagal menjalankan proses');
        }
    })
    .catch(err => {
        showToast('error', 'Error connecting to server');
    });
}

export function stopProcess(id) {
    fetch('/api/managed/stop', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id: id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            loadManagedProcesses();
        } else {
            showToast('error', res.message || 'Gagal menghentikan proses');
        }
    })
    .catch(err => {
        showToast('error', 'Error connecting to server');
    });
}

export function restartProcess(id) {
    fetch('/api/managed/restart', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id: id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            loadManagedProcesses();
        } else {
            showToast('error', res.message || 'Gagal merestart proses');
        }
    })
    .catch(err => {
        showToast('error', 'Error connecting to server');
    });
}

export function deleteProcess(id) {
    console.log('deleteProcess called with id:', id);
    if (!confirm('Apakah Anda yakin ingin menghapus proses managed ini?')) {
        console.log('deleteProcess cancelled by user');
        return;
    }
    console.log('Sending delete request for id:', id);
    fetch('/api/managed/delete', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id: id })
    })
    .then(res => {
        console.log('Delete response status:', res.status);
        return res.json();
    })
    .then(res => {
        console.log('Delete response JSON:', res);
        if (res.success) {
            showToast('success', res.message);
            loadManagedProcesses();
        } else {
            showToast('error', res.message || 'Gagal menghapus proses');
        }
    })
    .catch(err => {
        console.error('Delete request failed:', err);
        showToast('error', 'Error connecting to server');
    });
}

export function openDirPicker() {
    let currentVal = document.getElementById('proc-cwd').value.trim();
    dpCurrentPath = currentVal || '.';
    loadDirPickerFiles();
    const modal = document.getElementById('dir-picker-modal');
    if (modal) modal.classList.add('active');
}

export function closeDirPicker() {
    const modal = document.getElementById('dir-picker-modal');
    if (modal) modal.classList.remove('active');
}

export function confirmDirSelection() {
    const cwdInput = document.getElementById('proc-cwd');
    if (cwdInput) cwdInput.value = dpCurrentPath;
    closeDirPicker();
}

export function goUpDirPicker() {
    const p = dpCurrentPath;
    if (p === '/' || p === '' || p === '.') {
        return;
    }
    if (p.startsWith('/')) {
        const parts = p.replace(/\/+$/, '').split('/');
        parts.pop();
        dpCurrentPath = parts.join('/') || '/';
    } else {
        const parts = p.split('/');
        parts.pop();
        dpCurrentPath = parts.length > 0 ? parts.join('/') : '.';
    }
    loadDirPickerFiles();
}

export function loadDirPickerFiles() {
    const bc = document.getElementById('dp-breadcrumb');
    const dpList = document.getElementById('dp-dir-list');
    const selectedSpan = document.getElementById('dp-selected-path');

    if (selectedSpan) selectedSpan.innerText = dpCurrentPath;

    // Build Breadcrumb
    if (bc) {
        bc.innerHTML = '';
        const isAbsolute = dpCurrentPath.startsWith('/');
        const parts = dpCurrentPath.split('/').filter(x => x && x !== '.');

        let rootSpan = document.createElement('span');
        rootSpan.innerText = isAbsolute ? '/' : 'CWD';
        rootSpan.style.cursor = 'pointer';
        rootSpan.style.color = 'var(--accent-primary)';
        rootSpan.onclick = () => { dpCurrentPath = isAbsolute ? '/' : '.'; loadDirPickerFiles(); };
        bc.appendChild(rootSpan);

        let cumPath = isAbsolute ? '' : '.';
        parts.forEach((p) => {
            cumPath = isAbsolute ? (cumPath + '/' + p) : (cumPath + '/' + p);
            const pathTarget = cumPath;

            const sep = document.createElement('span');
            sep.innerText = '>';
            sep.style.margin = '0 4px';
            bc.appendChild(sep);

            const span = document.createElement('span');
            span.innerText = p;
            span.style.cursor = 'pointer';
            span.style.color = 'var(--accent-primary)';
            span.onclick = () => { dpCurrentPath = pathTarget; loadDirPickerFiles(); };
            bc.appendChild(span);
        });
    }

    // Load directories
    if (dpList) {
        dpList.innerHTML = '<div style="padding:20px; text-align:center; color:var(--text-muted);">Loading...</div>';

        fetch('/api/files/list?path=' + encodeURIComponent(dpCurrentPath))
            .then(res => res.json())
            .then(res => {
                dpList.innerHTML = '';

                // Add parent ".." directory row if not at root
                if (dpCurrentPath !== '.' && dpCurrentPath !== '/' && dpCurrentPath !== '') {
                    const row = document.createElement('div');
                    row.style = 'display:flex; align-items:center; padding:10px 20px; cursor:pointer; gap:10px; border-bottom:1px solid rgba(255,255,255,0.03);';
                    row.innerHTML = `
                        <i class="fa-solid fa-arrow-turn-up" style="color:var(--text-muted); transform:rotate(-90deg);"></i>
                        <span style="color:var(--text-muted); font-size:0.9rem; font-weight:bold;">..</span>
                    `;
                    row.onclick = () => {
                        goUpDirPicker();
                    };
                    dpList.appendChild(row);
                }

                if (res.success && res.data) {
                    const dirs = res.data.filter(item => item.is_dir);
                    if (dirs.length === 0) {
                        const emptyMsg = document.createElement('div');
                        emptyMsg.style = 'padding:20px; text-align:center; color:var(--text-muted); font-size:0.9rem;';
                        emptyMsg.innerText = 'Tidak ada subdirektori';
                        dpList.appendChild(emptyMsg);
                    } else {
                        dirs.forEach(item => {
                            const row = document.createElement('div');
                            row.style = 'display:flex; align-items:center; padding:10px 20px; cursor:pointer; gap:10px; border-bottom:1px solid rgba(255,255,255,0.03); transition: background 0.2s;';
                            row.innerHTML = `
                                <i class="fa-solid fa-folder folder" style="color:#f59e0b;"></i>
                                <span style="color:var(--text-main); font-size:0.9rem;">${item.name}</span>
                            `;
                            row.onmouseover = () => { row.style.background = 'rgba(255,255,255,0.05)'; };
                            row.onmouseout = () => { row.style.background = 'transparent'; };
                            row.onclick = () => {
                                dpCurrentPath = dpCurrentPath === '.' ? item.name :
                                                dpCurrentPath === '/' ? '/' + item.name :
                                                dpCurrentPath + '/' + item.name;
                                loadDirPickerFiles();
                            };
                            dpList.appendChild(row);
                        });
                    }
                } else {
                    dpList.innerHTML = `<div style="padding:20px; text-align:center; color:var(--danger);">${res.message || 'Gagal memuat direktori'}</div>`;
                }
            })
            .catch(err => {
                dpList.innerHTML = `<div style="padding:20px; text-align:center; color:var(--danger);">Error: ${err.toString()}</div>`;
            });
    }
}

export function openAddProcessModal() {
    const idVal = document.getElementById('proc-id-val');
    const nameInput = document.getElementById('proc-name');
    const cmdInput = document.getElementById('proc-command');
    const cwdInput = document.getElementById('proc-cwd');
    const arCheck = document.getElementById('proc-autorestart');
    const portInput = document.getElementById('proc-port');

    if (idVal) idVal.value = '';
    if (nameInput) nameInput.value = '';
    if (cmdInput) cmdInput.value = '';
    if (cwdInput) cwdInput.value = '';
    if (arCheck) arCheck.checked = true;
    if (portInput) portInput.value = '';

    const title = document.getElementById('modal-proc-title');
    const submitBtn = document.getElementById('btn-proc-submit');
    if (title) title.innerText = 'Add Managed Process';
    if (submitBtn) submitBtn.innerText = 'Add Process';
    
    const modal = document.getElementById('add-proc-modal');
    if (modal) modal.classList.add('active');
}

export function openEditProcessModal(id) {
    const proc = managedState.allManagedProcesses.find(p => p.id === id);
    if (!proc) return;

    const idVal = document.getElementById('proc-id-val');
    const nameInput = document.getElementById('proc-name');
    const cmdInput = document.getElementById('proc-command');
    const cwdInput = document.getElementById('proc-cwd');
    const arCheck = document.getElementById('proc-autorestart');
    const portInput = document.getElementById('proc-port');

    if (idVal) idVal.value = proc.id;
    if (nameInput) nameInput.value = proc.name;
    if (cmdInput) cmdInput.value = proc.command;
    if (cwdInput) cwdInput.value = proc.cwd;
    if (arCheck) arCheck.checked = proc.auto_restart;
    if (portInput) portInput.value = proc.port || '';

    const title = document.getElementById('modal-proc-title');
    const submitBtn = document.getElementById('btn-proc-submit');
    if (title) title.innerText = 'Edit Managed Process';
    if (submitBtn) submitBtn.innerText = 'Save Changes';

    const modal = document.getElementById('add-proc-modal');
    if (modal) modal.classList.add('active');
}

export function closeAddProcessModal() {
    const modal = document.getElementById('add-proc-modal');
    if (modal) modal.classList.remove('active');
}

export function submitAddProcess() {
    const idVal = document.getElementById('proc-id-val');
    const nameInput = document.getElementById('proc-name');
    const cmdInput = document.getElementById('proc-command');
    const cwdInput = document.getElementById('proc-cwd');
    const arCheck = document.getElementById('proc-autorestart');
    const portInput = document.getElementById('proc-port');

    const id = idVal ? idVal.value : '';
    const name = nameInput ? nameInput.value.trim() : '';
    const command = cmdInput ? cmdInput.value.trim() : '';
    const cwd = cwdInput ? (cwdInput.value.trim() || '.') : '.';
    const autoRestart = arCheck ? arCheck.checked : true;
    
    let port = null;
    if (portInput && portInput.value.trim()) {
        port = parseInt(portInput.value.trim(), 10);
        if (isNaN(port) || port <= 0 || port > 65535) {
            showToast('warning', 'Invalid port number (must be 1-65535)');
            return;
        }
    }

    if (!name || !command) {
        showToast('warning', 'Name and Command are required');
        return;
    }

    const env = {};

    const url = id ? '/api/managed/update' : '/api/managed/add';
    const body = id ? { id, name, command, cwd, env, auto_restart: autoRestart, port } : { name, command, cwd, env, auto_restart: autoRestart, port };

    fetch(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify(body)
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            closeAddProcessModal();
            loadManagedProcesses();
        } else {
            showToast('error', res.message || 'Gagal menyimpan proses');
        }
    })
    .catch(err => {
        showToast('error', 'Error connecting to server');
    });
}

export function viewProcessLogs(id, name) {
    activeLogProcessId = id;
    const title = document.getElementById('proc-logs-title');
    if (title) title.innerText = `Logs for: ${name}`;
    
    const viewport = document.getElementById('proc-logs-viewport');
    if (viewport) viewport.innerHTML = '<div style="color:#94a3b8; font-style:italic;">Loading logs...</div>';
    
    const modal = document.getElementById('proc-logs-modal');
    if (modal) modal.classList.add('active');

    loadLogs();
    if (logPollingInterval) clearInterval(logPollingInterval);
    logPollingInterval = setInterval(loadLogs, 2000);
}

export function loadLogs() {
    if (!activeLogProcessId) return;
    fetch(`/api/managed/logs?id=${activeLogProcessId}&lines=100`)
        .then(res => res.json())
        .then(res => {
            const viewport = document.getElementById('proc-logs-viewport');
            if (!viewport) return;
            if (res.data && res.data.length > 0) {
                const isAtBottom = viewport.scrollHeight - viewport.clientHeight <= viewport.scrollTop + 20;
                viewport.innerHTML = res.data.map(line => escapeHtml(line)).join('\n');
                if (isAtBottom) {
                    viewport.scrollTop = viewport.scrollHeight;
                }
            } else {
                viewport.innerHTML = '<div style="color:#94a3b8; font-style:italic;">No logs captured yet.</div>';
            }
        })
        .catch(err => {
            console.error('Failed to load process logs:', err);
        });
}

export function downloadProcessLogs() {
    if (!activeLogProcessId) return;
    window.open(`/api/processes/download_log?id=${activeLogProcessId}`, '_blank');
}

export function closeProcLogsModal() {
    const modal = document.getElementById('proc-logs-modal');
    if (modal) modal.classList.remove('active');
    activeLogProcessId = null;
    if (logPollingInterval) {
        clearInterval(logPollingInterval);
        logPollingInterval = null;
    }
}

export function toggleProcessDropdown(event, id) {
    if (event) {
        event.stopPropagation();
    }

    const targetMenu = document.getElementById(`menu-${id}`);
    const allMenus = document.querySelectorAll('.action-dropdown-menu');

    // Close all other open dropdowns and reset their fixed positioning
    allMenus.forEach(menu => {
        if (menu !== targetMenu) {
            menu.classList.remove('show');
            menu.style.position = '';
            menu.style.top = '';
            menu.style.left = '';
            menu.style.right = '';
            menu.style.bottom = '';
            if (menu.parentElement) {
                menu.parentElement.classList.remove('open-up');
            }
        }
    });

    if (targetMenu) {
        const isShowing = targetMenu.classList.toggle('show');

        if (isShowing) {
            // Use fixed positioning to escape overflow:hidden/auto clipping
            const btn = targetMenu.previousElementSibling || targetMenu.parentElement.querySelector('.action-dropdown-btn');
            if (btn) {
                const btnRect = btn.getBoundingClientRect();
                const menuWidth = 160; // min-width of the menu
                const menuHeight = 180; // approximate menu height

                // Position fixed, aligned to the right edge of the button
                targetMenu.style.position = 'fixed';
                targetMenu.style.right = 'auto';
                targetMenu.style.bottom = 'auto';

                const spaceBelow = window.innerHeight - btnRect.bottom;
                const spaceAbove = btnRect.top;

                if (spaceBelow < menuHeight && spaceAbove > spaceBelow) {
                    // Open upward
                    targetMenu.style.top = `${btnRect.top - menuHeight - 5}px`;
                    targetMenu.parentElement.classList.add('open-up');
                } else {
                    // Open downward
                    targetMenu.style.top = `${btnRect.bottom + 5}px`;
                    targetMenu.parentElement.classList.remove('open-up');
                }

                // Align right edge to button's right edge
                const leftPos = btnRect.right - menuWidth;
                targetMenu.style.left = `${Math.max(4, leftPos)}px`;
            }
        } else {
            // Reset fixed positioning on close
            targetMenu.style.position = '';
            targetMenu.style.top = '';
            targetMenu.style.left = '';
            targetMenu.style.right = '';
            targetMenu.style.bottom = '';
            if (targetMenu.parentElement) {
                targetMenu.parentElement.classList.remove('open-up');
            }
        }
    }
}
