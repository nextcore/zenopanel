import { getCSRFToken, escapeHtml } from './utils.js';
import { showToast } from './toast.js';

// Cache servers and databases locally
let registeredServers = [];
let userDatabases = [];
let activeSubTab = 'databases';

export function initDatabaseTab() {
    loadDatabaseServers();
    loadUserDatabases();
    updateConnectionSelector();
    loadDatabaseTables();
}

export function switchDatabaseSubTab(tabName) {
    activeSubTab = tabName;
    document.querySelectorAll('#tab-database .sub-tab-btn').forEach(btn => {
        btn.classList.remove('active');
        btn.style.background = 'transparent';
        btn.style.color = 'var(--text-muted)';
    });

    const activeBtn = document.getElementById(`subtab-db-${tabName}-btn`);
    if (activeBtn) {
        activeBtn.classList.add('active');
        activeBtn.style.background = 'rgba(59,130,246,0.1)';
        activeBtn.style.color = 'var(--accent-primary)';
    }

    const sections = ['databases', 'servers', 'console'];
    sections.forEach(sec => {
        const el = document.getElementById(`subtab-db-${sec}`);
        if (el) {
            el.style.display = (sec === tabName) ? 'block' : 'none';
        }
    });

    if (tabName === 'console') {
        updateConnectionSelector();
        loadDatabaseTables();
    }
}

// ==========================================
// 1. Database Servers Management
// ==========================================

export function loadDatabaseServers() {
    fetch('/api/database/servers')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                registeredServers = res.data;
                renderDatabaseServers();
                updateCreateDbServerDropdown();
                updateConnectionSelector();
            }
        })
        .catch(err => {
            console.error('Failed to load database servers:', err);
        });
}

function renderDatabaseServers() {
    const tbody = document.getElementById('db-servers-table-body');
    if (!tbody) return;
    
    if (registeredServers.length === 0) {
        tbody.innerHTML = `
            <tr>
                <td colspan="5" style="text-align:center; padding:32px; color:var(--text-muted);">
                    No database servers registered. Register one to manage databases.
                </td>
            </tr>
        `;
        return;
    }

    tbody.innerHTML = registeredServers.map(server => `
        <tr>
            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(server.name)}</td>
            <td><span class="badge ${server.driver === 'postgres' ? 'badge-running' : 'badge-starting'}">${server.driver.toUpperCase()}</span></td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${escapeHtml(server.host)}:${server.port}</td>
            <td>${escapeHtml(server.admin_user)}</td>
            <td style="text-align:right;">
                <button class="btn-action" onclick="deleteDatabaseServer(${server.id}, '${escapeHtml(server.name)}')" style="color:#ef4444; border-color:rgba(239,68,68,0.2);">
                    <i class="fa-solid fa-trash"></i> Delete
                </button>
            </td>
        </tr>
    `).join('');
}

export function openAddDbServerModal() {
    document.getElementById('db-server-name').value = '';
    document.getElementById('db-server-host').value = '127.0.0.1';
    document.getElementById('db-server-port').value = '3306';
    document.getElementById('db-server-admin-user').value = 'root';
    document.getElementById('db-server-admin-password').value = '';
    
    document.getElementById('db-server-driver').onchange = (e) => {
        const portField = document.getElementById('db-server-port');
        const userField = document.getElementById('db-server-admin-user');
        if (e.target.value === 'postgres') {
            portField.value = '5432';
            userField.value = 'postgres';
        } else {
            portField.value = '3306';
            userField.value = 'root';
        }
    };

    document.getElementById('add-db-server-modal').classList.add('show');
}

export function closeAddDbServerModal() {
    document.getElementById('add-db-server-modal').classList.remove('show');
}

export function submitAddDbServer() {
    const name = document.getElementById('db-server-name').value.trim();
    const driver = document.getElementById('db-server-driver').value;
    const host = document.getElementById('db-server-host').value.trim();
    const port = parseInt(document.getElementById('db-server-port').value) || 3306;
    const admin_user = document.getElementById('db-server-admin-user').value.trim();
    const admin_password = document.getElementById('db-server-admin-password').value;

    if (!name || !host || !admin_user) {
        showToast('error', 'Semua field wajib diisi');
        return;
    }

    showToast('info', 'Menguji koneksi ke server...');

    fetch('/api/database/servers', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ name, driver, host, port, admin_user, admin_password })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'Server database berhasil didaftarkan');
            closeAddDbServerModal();
            loadDatabaseServers();
        } else {
            showToast('error', res.message || 'Gagal mendaftarkan server');
        }
    })
    .catch(err => {
        showToast('error', 'API error: ' + err.toString());
    });
}

export function deleteDatabaseServer(id, name) {
    if (!confirm(`Apakah Anda yakin ingin menghapus registrasi server "${name}"? Semua database terkait di dalam ZenoPanel akan dilepas.`)) {
        return;
    }

    fetch('/api/database/servers', {
        method: 'DELETE',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'Server registrasi berhasil dihapus');
            loadDatabaseServers();
            loadUserDatabases();
        } else {
            showToast('error', res.message || 'Gagal menghapus server');
        }
    })
    .catch(err => {
        showToast('error', 'API error: ' + err.toString());
    });
}

// ==========================================
// 2. User Databases Lifecycle
// ==========================================

export function loadUserDatabases() {
    fetch('/api/database/list')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                userDatabases = res.data;
                renderUserDatabases();
                updateConnectionSelector();
            }
        })
        .catch(err => {
            console.error('Failed to load user databases:', err);
        });
}

function renderUserDatabases() {
    const tbody = document.getElementById('db-databases-table-body');
    if (!tbody) return;

    if (userDatabases.length === 0) {
        tbody.innerHTML = `
            <tr>
                <td colspan="7" style="text-align:center; padding:32px; color:var(--text-muted);">
                    No databases created yet. Click "Create Database" to start.
                </td>
            </tr>
        `;
        return;
    }

    tbody.innerHTML = userDatabases.map(db => `
        <tr>
            <td style="font-weight:600; color:var(--accent-primary);">${escapeHtml(db.db_name)}</td>
            <td>
                <div style="font-weight:500; font-size:0.85rem;">${escapeHtml(db.server_name)}</div>
                <div style="font-size:0.75rem; color:var(--text-muted); text-transform:uppercase;">${escapeHtml(db.server_driver)}</div>
            </td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${escapeHtml(db.db_user)}</td>
            <td>
                <div style="display:flex; align-items:center; gap:8px;">
                    <span id="pw-mask-${db.id}" style="font-family:var(--font-code); font-size:0.85rem;">••••••••</span>
                    <span id="pw-text-${db.id}" style="font-family:var(--font-code); font-size:0.85rem; display:none;">${escapeHtml(db.db_password)}</span>
                    <button class="btn-action" onclick="togglePasswordVisibility(${db.id})" style="padding:2px 6px; font-size:0.75rem;">
                        <i class="fa-solid fa-eye" id="pw-icon-${db.id}"></i>
                    </button>
                </div>
            </td>
            <td><span class="badge ${db.access_type === 'local' ? 'badge-sleeping' : 'badge-running'}">${db.access_type.toUpperCase()}</span></td>
            <td style="font-size:0.8rem; max-width:200px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;" title="${escapeHtml(db.description || '')}">${escapeHtml(db.description || '-')}</td>
            <td style="text-align:right;">
                <div style="display:flex; justify-content:flex-end; gap:6px;">
                    <button class="btn-action" onclick="openConsoleForDb('${escapeHtml(db.db_name)}')" style="color:var(--accent-primary); border-color:rgba(59,130,246,0.2);">
                        <i class="fa-solid fa-code"></i> Console
                    </button>
                    <button class="btn-action" onclick="openChangeDbPasswordModal(${db.id})" style="color:var(--text-main);">
                        <i class="fa-solid fa-key"></i> Pass
                    </button>
                    <button class="btn-action" onclick="deleteUserDatabase(${db.id}, '${escapeHtml(db.db_name)}')" style="color:#ef4444; border-color:rgba(239,68,68,0.2);">
                        <i class="fa-solid fa-trash"></i> Drop
                    </button>
                </div>
            </td>
        </tr>
    `).join('');
}

window.togglePasswordVisibility = function(id) {
    const mask = document.getElementById(`pw-mask-${id}`);
    const text = document.getElementById(`pw-text-${id}`);
    const icon = document.getElementById(`pw-icon-${id}`);
    
    if (mask.style.display === 'none') {
        mask.style.display = 'inline';
        text.style.display = 'none';
        icon.className = 'fa-solid fa-eye';
    } else {
        mask.style.display = 'none';
        text.style.display = 'inline';
        icon.className = 'fa-solid fa-eye-slash';
    }
};

export function openAddDatabaseModal() {
    if (registeredServers.length === 0) {
        showToast('error', 'Silakan daftarkan server database terlebih dahulu di tab Database Servers');
        return;
    }
    
    document.getElementById('db-create-name').value = '';
    document.getElementById('db-create-user').value = '';
    document.getElementById('db-create-password').value = generateSecurePassword();
    document.getElementById('db-create-desc').value = '';
    
    // Auto-fill user field when name field changes
    document.getElementById('db-create-name').oninput = (e) => {
        document.getElementById('db-create-user').value = e.target.value.toLowerCase().replace(/[^a-z0-9_]/g, '');
    };

    document.getElementById('create-db-modal').classList.add('show');
}

export function closeAddDatabaseModal() {
    document.getElementById('create-db-modal').classList.remove('remove');
    document.getElementById('create-db-modal').classList.remove('show');
}

export function generateRandomDbPassword() {
    document.getElementById('db-create-password').value = generateSecurePassword();
}

export function submitCreateDatabase() {
    const server_id = parseInt(document.getElementById('db-create-server-select').value);
    const db_name = document.getElementById('db-create-name').value.trim();
    const db_user = document.getElementById('db-create-user').value.trim();
    const db_password = document.getElementById('db-create-password').value;
    const access_type = document.getElementById('db-create-access').value;
    const description = document.getElementById('db-create-desc').value.trim();

    if (!server_id || !db_name || !db_user || !db_password) {
        showToast('error', 'Semua field utama wajib diisi');
        return;
    }

    const selectedServer = registeredServers.find(s => s.id === server_id);
    const host_ip = selectedServer ? selectedServer.host : '127.0.0.1';
    const host_port = selectedServer ? selectedServer.port : 3306;

    showToast('info', 'Membuat database & user di server target...');

    fetch('/api/database/create', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ server_id, db_name, db_user, db_password, access_type, description, host_ip, host_port })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'Database & User berhasil dibuat');
            closeAddDatabaseModal();
            loadUserDatabases();
        } else {
            showToast('error', res.message || 'Gagal membuat database');
        }
    })
    .catch(err => {
        showToast('error', 'API error: ' + err.toString());
    });
}

export function deleteUserDatabase(id, name) {
    if (!confirm(`Apakah Anda yakin ingin melakukan DROP pada database "${name}"? Semua data di dalamnya akan terhapus selamanya.`)) {
        return;
    }

    fetch('/api/database/delete', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'Database berhasil di-drop');
            loadUserDatabases();
        } else {
            showToast('error', res.message || 'Gagal drop database');
        }
    })
    .catch(err => {
        showToast('error', 'API error: ' + err.toString());
    });
}

export function openChangeDbPasswordModal(id) {
    document.getElementById('change-db-id').value = id;
    document.getElementById('change-db-password-field').value = generateSecurePassword();
    document.getElementById('change-db-password-modal').classList.add('show');
}

export function closeChangeDbPasswordModal() {
    document.getElementById('change-db-password-modal').classList.remove('show');
}

export function generateRandomDbPasswordChange() {
    document.getElementById('change-db-password-field').value = generateSecurePassword();
}

export function submitChangeDbPassword() {
    const id = parseInt(document.getElementById('change-db-id').value);
    const password = document.getElementById('change-db-password-field').value;

    if (!password) {
        showToast('error', 'Password tidak boleh kosong');
        return;
    }

    fetch('/api/database/change-password', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id, password })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'Password database user berhasil diperbarui');
            closeChangeDbPasswordModal();
            loadUserDatabases();
        } else {
            showToast('error', res.message || 'Gagal mengubah password');
        }
    })
    .catch(err => {
        showToast('error', 'API error: ' + err.toString());
    });
}

// ==========================================
// 3. SQL Console Explorer
// ==========================================

export function openConsoleForDb(dbName) {
    switchDatabaseSubTab('console');
    const select = document.getElementById('db-console-connection-select');
    if (select) {
        select.value = dbName;
        onConsoleConnectionChange();
    }
}

export function onConsoleConnectionChange() {
    loadDatabaseTables();
    const resultsContainer = document.getElementById('db-results-container');
    if (resultsContainer) {
        resultsContainer.innerHTML = '<div style="padding:32px; text-align:center; color:var(--text-muted); font-size:0.85rem;">Run a query above to see results.</div>';
    }
    const sqlInput = document.getElementById('sql-query-input');
    if (sqlInput) sqlInput.value = '';
}

export function loadDatabaseTables() {
    const select = document.getElementById('db-console-connection-select');
    const connection = select ? select.value : 'default';

    fetch(`/api/database/tables?connection=${encodeURIComponent(connection)}`)
        .then(res => res.json())
        .then(res => {
            const list = document.getElementById('db-tables-list');
            if (!list) return;
            list.innerHTML = '';
            
            const badge = document.getElementById('db-driver-badge');
            if (badge) badge.innerText = (res.driver || 'SQLITE').toUpperCase();

            if (res.success && res.data) {
                if (res.data.length === 0) {
                    list.innerHTML = '<div style="padding:16px; text-align:center; color:var(--text-muted); font-size:0.8rem;">No tables found.</div>';
                    return;
                }

                res.data.forEach(item => {
                    const tableName = item.name || Object.values(item)[0];
                    const div = document.createElement('div');
                    div.className = 'db-table-item';
                    div.style.padding = '8px 10px';
                    div.style.borderRadius = '4px';
                    div.style.cursor = 'pointer';
                    div.style.fontSize = '0.82rem';
                    div.style.color = 'var(--text-muted)';
                    div.style.display = 'flex';
                    div.style.alignItems = 'center';
                    div.style.gap = '8px';
                    div.innerHTML = `<i class="fa-solid fa-table" style="color:var(--accent-primary); font-size:0.75rem;"></i> <span>${tableName}</span>`;
                    
                    div.onmouseover = () => { div.style.background = 'rgba(255,255,255,0.05)'; div.style.color = 'var(--text-main)'; };
                    div.onmouseout = () => { div.style.background = 'transparent'; div.style.color = 'var(--text-muted)'; };
                    
                    div.onclick = () => {
                        const q = `SELECT * FROM ${tableName} LIMIT 10;`;
                        const sqlQueryInput = document.getElementById('sql-query-input');
                        const sqlIsSelect = document.getElementById('sql-is-select');
                        if (sqlQueryInput) sqlQueryInput.value = q;
                        if (sqlIsSelect) sqlIsSelect.checked = true;
                    };
                    list.appendChild(div);
                });
            } else {
                list.innerHTML = `<div style="padding:16px; text-align:center; color:var(--danger); font-size:0.8rem;">${res.message || 'Error loading tables'}</div>`;
            }
        })
        .catch(err => {
            const list = document.getElementById('db-tables-list');
            if (list) {
                list.innerHTML = '<div style="padding:16px; text-align:center; color:var(--danger); font-size:0.8rem;">Error loading tables</div>';
            }
        });
}

export function runSqlQuery() {
    const select = document.getElementById('db-console-connection-select');
    const connection = select ? select.value : 'default';
    const sqlInput = document.getElementById('sql-query-input');
    const isSelectInput = document.getElementById('sql-is-select');
    const sql = sqlInput ? sqlInput.value : '';
    const isSelect = isSelectInput ? isSelectInput.checked : true;

    if (!sql.trim()) {
        showToast('error', 'Masukkan query SQL terlebih dahulu');
        return;
    }

    const resultsContainer = document.getElementById('db-results-container');
    if (resultsContainer) {
        resultsContainer.innerHTML = '<div style="padding:32px; text-align:center;"><i class="fa-solid fa-spinner fa-spin" style="font-size:1.5rem; color:var(--accent-primary)"></i><p style="margin-top:10px; color:var(--text-muted)">Running query...</p></div>';
    }

    fetch(`/api/database/query?connection=${encodeURIComponent(connection)}`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ sql: sql, is_select: isSelect })
    })
    .then(res => res.json())
    .then(res => {
        if (!resultsContainer) return;
        if (res.success) {
            showToast('success', 'Query dijalankan sukses');
            if (res.type === 'select') {
                renderDbSelectResult(res.data);
            } else {
                resultsContainer.innerHTML = `
                    <div style="padding: 24px; background: rgba(16, 185, 129, 0.08); border-radius: 8px; border: 1px solid rgba(16, 185, 129, 0.2); line-height: 1.6;">
                        <div style="color:var(--success); font-weight:600; font-size:1rem; margin-bottom:8px;"><i class="fa-solid fa-circle-check"></i> Command SQL Berhasil Dijalankan!</div>
                        <div style="font-size:0.85rem;">Rows Affected: <strong style="font-size:0.95rem; font-family:var(--font-code);">${res.affected}</strong></div>
                        <div style="font-size:0.85rem;">Last Inserted ID: <strong style="font-size:0.95rem; font-family:var(--font-code);">${res.last_id}</strong></div>
                    </div>
                `;
            }
        } else {
            resultsContainer.innerHTML = `
                <div style="padding: 24px; background: rgba(239, 68, 68, 0.08); border-radius: 8px; border: 1px solid rgba(239, 68, 68, 0.2); line-height: 1.5;">
                    <div style="color:var(--danger); font-weight:600; font-size:1rem; margin-bottom:8px;"><i class="fa-solid fa-triangle-exclamation"></i> SQL Execution Failed!</div>
                    <div style="font-family:var(--font-code); font-size:0.82rem; color:#fca5a5; word-break: break-all;">${res.message || 'Unknown database error'}</div>
                </div>
            `;
        }
    })
    .catch(err => {
        if (resultsContainer) {
            resultsContainer.innerHTML = `<div style="padding:32px; text-align:center; color:var(--danger)">API request failed: ${err.toString()}</div>`;
        }
    });
}

export function renderDbSelectResult(data) {
    const container = document.getElementById('db-results-container');
    if (!container) return;
    if (!data || data.length === 0) {
        container.innerHTML = '<div style="padding:32px; text-align:center; color:var(--text-muted)">Query completed successfully, but returned 0 rows.</div>';
        return;
    }

    const keys = Object.keys(data[0]);
    
    let tableHtml = `
        <table class="data-table" style="width:100%">
            <thead>
                <tr>
                    ${keys.map(k => `<th>${escapeHtml(k)}</th>`).join('')}
                </tr>
            </thead>
            <tbody>
                ${data.map(row => `
                    <tr>
                        ${keys.map(k => `<td>${row[k] !== null ? escapeHtml(row[k].toString()) : '<em style="opacity:0.4">NULL</em>'}</td>`).join('')}
                    </tr>
                `).join('')}
            </tbody>
        </table>
    `;
    container.innerHTML = tableHtml;
}

// ==========================================
// 4. Utility Dropdowns & Helpers
// ==========================================

function updateCreateDbServerDropdown() {
    const select = document.getElementById('db-create-server-select');
    if (!select) return;
    
    select.innerHTML = registeredServers.map(server => `
        <option value="${server.id}">${escapeHtml(server.name)} (${server.driver.toUpperCase()})</option>
    `).join('');
}

function updateConnectionSelector() {
    const select = document.getElementById('db-console-connection-select');
    if (!select) return;

    const currentVal = select.value;
    
    let optionsHtml = '<option value="default">Default Panel DB (SQLite)</option>';
    
    // Add registered raw servers
    registeredServers.forEach(server => {
        optionsHtml += `<option value="${escapeHtml(server.name)}">${escapeHtml(server.name)} (Server Admin - ${server.driver.toUpperCase()})</option>`;
    });

    // Add created user databases specifically
    userDatabases.forEach(db => {
        optionsHtml += `<option value="${escapeHtml(db.db_name)}">${escapeHtml(db.db_name)} (User DB - ${db.server_driver.toUpperCase()})</option>`;
    });

    select.innerHTML = optionsHtml;
    
    // Maintain selection if still valid
    if (select.querySelector(`option[value="${currentVal}"]`)) {
        select.value = currentVal;
    }
}

function generateSecurePassword() {
    const chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+~";
    let pass = "";
    for (let i = 0; i < 16; i++) {
        pass += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return pass;
}

// Bind to window to allow HTML button trigger
window.switchDatabaseSubTab = switchDatabaseSubTab;
window.openAddDbServerModal = openAddDbServerModal;
window.closeAddDbServerModal = closeAddDbServerModal;
window.submitAddDbServer = submitAddDbServer;
window.deleteDatabaseServer = deleteDatabaseServer;
window.openAddDatabaseModal = openAddDatabaseModal;
window.closeAddDatabaseModal = closeAddDatabaseModal;
window.generateRandomDbPassword = generateRandomDbPassword;
window.submitCreateDatabase = submitCreateDatabase;
window.deleteUserDatabase = deleteUserDatabase;
window.openChangeDbPasswordModal = openChangeDbPasswordModal;
window.closeChangeDbPasswordModal = closeChangeDbPasswordModal;
window.generateRandomDbPasswordChange = generateRandomDbPasswordChange;
window.submitChangeDbPassword = submitChangeDbPassword;
window.onConsoleConnectionChange = onConsoleConnectionChange;
window.openConsoleForDb = openConsoleForDb;
