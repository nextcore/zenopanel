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
                <td colspan="8" style="text-align:center; padding:30px; color:var(--text-muted);">
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

        tr.innerHTML = `
            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(proc.name)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; max-width:250px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;" title="${escapeHtml(proc.command)}">${escapeHtml(proc.command)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:#10b981; font-weight:600;">${cpuVal}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:#6366f1; font-weight:600;">${memVal}</td>
            <td style="font-size:0.85rem; color:var(--text-muted);">${escapeHtml(proc.cwd)}</td>
            <td>${statusBadge}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${proc.pid ? proc.pid : '-'}</td>
            <td style="text-align:right;">
                <div style="display:inline-flex; gap:8px;">
                    ${proc.status === 'running' || proc.status === 'starting' ? `
                        <button class="btn-action" onclick="stopProcess('${proc.id}')" title="Stop Process" style="padding:4px 8px; font-size:0.75rem; color:#ef4444; border-color:rgba(239,68,68,0.2); background:rgba(239,68,68,0.05);">
                            <i class="fa-solid fa-stop"></i> Stop
                        </button>
                    ` : `
                        <button class="btn-action" onclick="startProcess('${proc.id}')" title="Start Process" style="padding:4px 8px; font-size:0.75rem; color:var(--success); border-color:rgba(16,185,129,0.2); background:rgba(16,185,129,0.05);">
                            <i class="fa-solid fa-play"></i> Start
                        </button>
                    `}
                    <button class="btn-action" onclick="restartProcess('${proc.id}')" title="Restart Process" style="padding:4px 8px; font-size:0.75rem; color:var(--accent-primary); border-color:rgba(59,130,246,0.2); background:rgba(59,130,246,0.05);">
                        <i class="fa-solid fa-rotate-right"></i> Restart
                    </button>
                    <button class="btn-action" onclick="openEditProcessModal('${proc.id}')" title="Edit Process" style="padding:4px 8px; font-size:0.75rem; color:var(--warning); border-color:rgba(245,158,11,0.2); background:rgba(245,158,11,0.05);">
                        <i class="fa-solid fa-pen-to-square"></i> Edit
                    </button>
                    <button class="btn-action" onclick="viewProcessLogs('${proc.id}', '${escapeHtml(proc.name)}')" title="View Logs" style="padding:4px 8px; font-size:0.75rem;">
                        <i class="fa-solid fa-terminal"></i> Logs
                    </button>
                    <button class="btn-action" onclick="deleteProcess('${proc.id}')" title="Delete Process" style="padding:4px 8px; font-size:0.75rem; color:#ef4444; border-color:transparent; background:transparent;">
                        <i class="fa-solid fa-trash-can"></i>
                    </button>
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

    if (idVal) idVal.value = '';
    if (nameInput) nameInput.value = '';
    if (cmdInput) cmdInput.value = '';
    if (cwdInput) cwdInput.value = '';
    if (arCheck) arCheck.checked = true;

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

    if (idVal) idVal.value = proc.id;
    if (nameInput) nameInput.value = proc.name;
    if (cmdInput) cmdInput.value = proc.command;
    if (cwdInput) cwdInput.value = proc.cwd;
    if (arCheck) arCheck.checked = proc.auto_restart;

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

    const id = idVal ? idVal.value : '';
    const name = nameInput ? nameInput.value.trim() : '';
    const command = cmdInput ? cmdInput.value.trim() : '';
    const cwd = cwdInput ? (cwdInput.value.trim() || '.') : '.';
    const autoRestart = arCheck ? arCheck.checked : true;

    if (!name || !command) {
        showToast('warning', 'Name and Command are required');
        return;
    }

    const env = {};

    const url = id ? '/api/managed/update' : '/api/managed/add';
    const body = id ? { id, name, command, cwd, env, auto_restart: autoRestart } : { name, command, cwd, env, auto_restart: autoRestart };

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
