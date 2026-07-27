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
    loadDatabaseBackups();
    loadDatabaseBackupSchedules();
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

    const sections = ['databases', 'servers', 'console', 'backups'];
    sections.forEach(sec => {
        const el = document.getElementById(`subtab-db-${sec}`);
        if (el) {
            el.style.display = (sec === tabName) ? 'block' : 'none';
        }
    });

    if (tabName === 'console') {
        updateConnectionSelector();
        loadDatabaseTables();
    } else if (tabName === 'backups') {
        loadDatabaseBackups();
        loadDatabaseBackupSchedules();
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
                <td colspan="8" style="text-align:center; padding:32px; color:var(--text-muted);">
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
            <td id="db-server-uptime-${server.name}" style="font-size:0.8rem; color:var(--text-muted);">-</td>
            <td id="db-server-connections-${server.name}" style="font-size:0.85rem; font-weight:600; color:var(--text-muted);">-</td>
            <td>
                ${server.pool_enabled === 1 ? `
                    <span class="badge badge-running" style="cursor:pointer; user-select:none;" onclick="toggleDatabasePool(${server.id}, 0)" title="Click to disable connection pool">
                        <i class="fa-solid fa-water"></i> Port ${server.pool_port}
                    </span>
                ` : `
                    <span class="badge badge-sleeping" style="cursor:pointer; user-select:none;" onclick="toggleDatabasePool(${server.id}, 1)" title="Click to enable connection pool (ProxySQL for MySQL, PgBouncer for PostgreSQL)">
                        <i class="fa-solid fa-water-ladder"></i> Disabled
                    </span>
                `}
            </td>
            <td>${escapeHtml(server.admin_user)}</td>
            <td>
                <span class="badge ${server.is_remote === 1 ? 'badge-running' : 'badge-sleeping'}" style="cursor:pointer; user-select:none;" onclick="toggleDatabaseRemoteAccess(${server.id}, ${server.is_remote === 1 ? 0 : 1})">
                    <i class="fa-solid ${server.is_remote === 1 ? 'fa-globe' : 'fa-lock'}"></i> ${server.is_remote === 1 ? 'Remote' : 'Local Only'}
                </span>
            </td>
            <td style="text-align:right;">
                <div style="display:flex; justify-content:flex-end; gap:6px;">
                    <button onclick="openDatabaseConfigModal(${server.id}, '${escapeHtml(server.name)}', '${server.driver}')" style="background:rgba(234,179,8,0.15); border:none; color:#facc15; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Configure parameters">
                        <i class="fa-solid fa-sliders"></i>
                    </button>
                    <button onclick="openDatabaseLogsModal('${escapeHtml(server.name)}')" style="background:rgba(59,130,246,0.15); border:none; color:#60a5fa; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Database Logs">
                        <i class="fa-solid fa-terminal"></i>
                    </button>
                    <button onclick="deleteDatabaseServer(${server.id}, '${escapeHtml(server.name)}')" style="background:rgba(239,68,68,0.15); border:none; color:#f87171; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Delete Server">
                        <i class="fa-solid fa-trash-can"></i>
                    </button>
                </div>
            </td>
        </tr>
    `).join('');

    // Dynamically fetch live status for each database server container
    registeredServers.forEach(server => {
        fetch(`/api/database/servers/status?name=${encodeURIComponent(server.name)}&driver=${encodeURIComponent(server.driver)}`)
            .then(res => res.json())
            .then(res => {
                const elUpt = document.getElementById(`db-server-uptime-${server.name}`);
                const elConn = document.getElementById(`db-server-connections-${server.name}`);
                if (res.success) {
                    if (elUpt) elUpt.innerText = formatUptime(res.uptime);
                    if (elConn) elConn.innerText = res.connections;
                } else {
                    if (elUpt) elUpt.innerText = 'Offline';
                    if (elConn) elConn.innerText = 'Offline';
                }
            })
            .catch(() => {
                const elUpt = document.getElementById(`db-server-uptime-${server.name}`);
                const elConn = document.getElementById(`db-server-connections-${server.name}`);
                if (elUpt) elUpt.innerText = 'Offline';
                if (elConn) elConn.innerText = 'Offline';
            });
    });
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

    document.getElementById('add-db-server-modal').classList.add('active');
}

export function closeAddDbServerModal() {
    document.getElementById('add-db-server-modal').classList.remove('active');
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

    fetch('/api/database/servers/delete', {
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
                <td colspan="9" style="text-align:center; padding:32px; color:var(--text-muted);">
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
            <td id="db-size-${db.db_name}" style="color:var(--text-muted); font-size:0.85rem;">-</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${escapeHtml(db.db_user)}</td>
            <td>
                <div style="display:flex; align-items:center; gap:8px;">
                    <span id="pw-mask-${db.id}" style="font-family:var(--font-code); font-size:0.85rem;">••••••••</span>
                    <span id="pw-text-${db.id}" style="font-family:var(--font-code); font-size:0.85rem; display:none;">${escapeHtml(db.db_password)}</span>
                    <button onclick="togglePasswordVisibility(${db.id})" style="background:rgba(255,255,255,0.08); border:none; color:#e4e4e7; padding:4px 8px; border-radius:4px; cursor:pointer; transition:all 0.2s;" title="Toggle Password">
                        <i class="fa-solid fa-eye" id="pw-icon-${db.id}"></i>
                    </button>
                </div>
            </td>
            <td style="font-family:var(--font-code); font-size:0.8rem; color:var(--text-muted);">${escapeHtml(db.charset || 'utf8mb4')}</td>
            <td><span class="badge ${db.access_type === 'local' ? 'badge-sleeping' : 'badge-running'}">${db.access_type.toUpperCase()}</span></td>
            <td style="font-size:0.8rem; max-width:160px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;" title="${escapeHtml(db.description || '')}">${escapeHtml(db.description || '-')}</td>
            <td style="text-align:right;">
                <div style="display:flex; justify-content:flex-end; gap:6px;">
                    <button onclick="openDatabaseMaintenanceModal('${escapeHtml(db.db_name)}', '${escapeHtml(db.server_name)}', '${db.server_driver}')" style="background:rgba(234,179,8,0.15); border:none; color:#facc15; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Database Maintenance">
                        <i class="fa-solid fa-screwdriver-wrench"></i>
                    </button>
                    <button onclick="openManageDbUsersModal(${db.id}, '${escapeHtml(db.db_name)}')" style="background:rgba(168,85,247,0.15); border:none; color:#c084fc; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Manage Users">
                        <i class="fa-solid fa-users"></i>
                    </button>
                    <button onclick="openConsoleForDb('${escapeHtml(db.db_name)}')" style="background:rgba(59,130,246,0.15); border:none; color:#60a5fa; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="SQL Console">
                        <i class="fa-solid fa-code"></i>
                    </button>
                    <button onclick="openImportDatabaseModal('${escapeHtml(db.db_name)}')" style="background:rgba(168,85,247,0.15); border:none; color:#c084fc; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Import SQL File">
                        <i class="fa-solid fa-file-import"></i>
                    </button>
                    <button onclick="openChangeDbPasswordModal(${db.id})" style="background:rgba(255,255,255,0.08); border:none; color:#e4e4e7; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Change Password">
                        <i class="fa-solid fa-key"></i>
                    </button>
                    <button onclick="deleteUserDatabase(${db.id}, '${escapeHtml(db.db_name)}')" style="background:rgba(239,68,68,0.15); border:none; color:#f87171; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Delete Database">
                        <i class="fa-solid fa-trash-can"></i>
                    </button>
                </div>
            </td>
        </tr>
    `).join('');

    // Dynamically fetch storage size for each user database
    userDatabases.forEach(db => {
        fetch(`/api/database/size?server_name=${encodeURIComponent(db.server_name)}&db_name=${encodeURIComponent(db.db_name)}&driver=${encodeURIComponent(db.server_driver)}`)
            .then(res => res.json())
            .then(res => {
                const el = document.getElementById(`db-size-${db.db_name}`);
                if (el) {
                    el.innerText = res.success ? formatBytes(res.size) : 'Error';
                }
            })
            .catch(() => {
                const el = document.getElementById(`db-size-${db.db_name}`);
                if (el) el.innerText = 'Error';
            });
    });
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
    
    // Reset charset/collation to defaults
    const charsetSel = document.getElementById('db-create-charset');
    if (charsetSel) charsetSel.value = 'utf8mb4';
    onDbCharsetChange();

    // Auto-fill user field when name field changes
    document.getElementById('db-create-name').oninput = (e) => {
        document.getElementById('db-create-user').value = e.target.value.toLowerCase().replace(/[^a-z0-9_]/g, '');
    };

    // Show/hide charset group depending on selected server driver
    document.getElementById('db-create-server-select').onchange = updateCharsetVisibility;
    updateCharsetVisibility();

    document.getElementById('create-db-modal').classList.add('active');
}

export function closeAddDatabaseModal() {
    document.getElementById('create-db-modal').classList.remove('active');
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

    // Charset / collation / encoding
    const selectedServer = registeredServers.find(s => s.id === server_id);
    const isPostgres = selectedServer && selectedServer.driver === 'postgres';
    let charset = 'utf8mb4';
    let collation = 'utf8mb4_unicode_ci';
    if (isPostgres) {
        charset = document.getElementById('db-create-encoding')?.value || 'UTF8';
        collation = '';
    } else {
        charset = document.getElementById('db-create-charset')?.value || 'utf8mb4';
        collation = document.getElementById('db-create-collation')?.value || 'utf8mb4_unicode_ci';
    }

    if (!server_id || !db_name || !db_user || !db_password) {
        showToast('error', 'Semua field utama wajib diisi');
        return;
    }

    const host_ip = selectedServer ? selectedServer.host : '127.0.0.1';
    const host_port = selectedServer ? selectedServer.port : 3306;

    showToast('info', 'Membuat database & user di server target...');

    fetch('/api/database/create', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ server_id, db_name, db_user, db_password, access_type, description, host_ip, host_port, charset, collation })
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
    document.getElementById('change-db-password-modal').classList.add('active');
}

export function closeChangeDbPasswordModal() {
    document.getElementById('change-db-password-modal').classList.remove('active');
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

let sqlMonacoInstance = null;
let dbConfigMonacoInstance = null;

function getOrInitDatabaseMonaco(callback) {
    if (window.monaco) {
        callback();
        return;
    }
    const loaderScript = document.createElement('script');
    loaderScript.src = '/public/js/monaco/vs/loader.js';
    loaderScript.onload = () => {
        require.config({ paths: { vs: '/public/js/monaco/vs' } });
        require(['vs/editor/editor.main'], () => {
            callback();
        });
    };
    document.head.appendChild(loaderScript);
}

function getSqlContent() {
    if (sqlMonacoInstance) {
        return sqlMonacoInstance.getValue();
    }
    const textarea = document.getElementById('sql-query-input');
    return textarea ? textarea.value : '';
}

function setSqlContent(val) {
    const textarea = document.getElementById('sql-query-input');
    if (textarea) textarea.value = val;
    
    const container = document.getElementById('sql-monaco-container');
    if (container) {
        getOrInitDatabaseMonaco(() => {
            if (!sqlMonacoInstance) {
                sqlMonacoInstance = monaco.editor.create(container, {
                    value: val,
                    language: 'sql',
                    theme: 'vs-dark',
                    automaticLayout: true,
                    fontSize: 13,
                    fontFamily: 'var(--font-code)',
                    minimap: { enabled: false }
                });
            } else {
                sqlMonacoInstance.setValue(val);
            }
        });
    }
}

function getDbConfigContent() {
    if (dbConfigMonacoInstance) {
        return dbConfigMonacoInstance.getValue();
    }
    const textarea = document.getElementById('db-config-raw-content');
    return textarea ? textarea.value : '';
}

function setDbConfigContent(val) {
    const textarea = document.getElementById('db-config-raw-content');
    if (textarea) textarea.value = val;

    const container = document.getElementById('db-config-monaco-container');
    if (container) {
        getOrInitDatabaseMonaco(() => {
            if (!dbConfigMonacoInstance) {
                dbConfigMonacoInstance = monaco.editor.create(container, {
                    value: val,
                    language: 'ini',
                    theme: 'vs-dark',
                    automaticLayout: true,
                    fontSize: 13,
                    fontFamily: 'var(--font-code)',
                    minimap: { enabled: false }
                });
            } else {
                dbConfigMonacoInstance.setValue(val);
            }
        });
    }
}

export function onConsoleConnectionChange() {
    loadDatabaseTables();
    const resultsContainer = document.getElementById('db-results-container');
    if (resultsContainer) {
        resultsContainer.innerHTML = '<div style="padding:32px; text-align:center; color:var(--text-muted); font-size:0.85rem;">Run a query above to see results.</div>';
    }
    setSqlContent('');
}

export function loadDatabaseTables() {
    const select = document.getElementById('db-console-connection-select');
    const connection = select ? select.value : '';

    const list = document.getElementById('db-tables-list');
    if (!list) return;

    if (!connection) {
        list.innerHTML = '<div style="padding:16px; text-align:center; color:var(--text-muted); font-size:0.8rem;">Pilih koneksi terlebih dahulu.</div>';
        const badge = document.getElementById('db-driver-badge');
        if (badge) badge.innerText = 'NONE';
        return;
    }

    fetch(`/api/database/tables?connection=${encodeURIComponent(connection)}`)
        .then(res => res.json())
        .then(res => {
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
                        setSqlContent(q);
                        const sqlIsSelect = document.getElementById('sql-is-select');
                        if (sqlIsSelect) sqlIsSelect.checked = true;
                    };
                    list.appendChild(div);
                });
            } else {
                list.innerHTML = `<div style="padding:16px; text-align:center; color:var(--danger); font-size:0.8rem;">${res.message || 'Error loading tables'}</div>`;
            }
        })
        .catch(err => {
            if (list) {
                list.innerHTML = '<div style="padding:16px; text-align:center; color:var(--danger); font-size:0.8rem;">Error loading tables</div>';
            }
        });
}

export function runSqlQuery() {
    const select = document.getElementById('db-console-connection-select');
    const connection = select ? select.value : '';
    const isSelectInput = document.getElementById('sql-is-select');
    const sql = getSqlContent();
    const isSelect = isSelectInput ? isSelectInput.checked : true;

    if (!connection) {
        showToast('error', 'Silakan pilih koneksi database terlebih dahulu');
        return;
    }

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
        <table class="data-table db-results-table" style="width:100%">
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
// 4. Charset / Collation Helpers
// ==========================================

const COLLATIONS = {
    'utf8mb4': ['utf8mb4_unicode_ci', 'utf8mb4_general_ci', 'utf8mb4_0900_ai_ci'],
    'utf8':    ['utf8_general_ci', 'utf8_unicode_ci'],
    'latin1':  ['latin1_swedish_ci', 'latin1_general_ci', 'latin1_bin'],
    'ascii':   ['ascii_general_ci', 'ascii_bin'],
};

export function onDbCharsetChange() {
    const charset = document.getElementById('db-create-charset')?.value || 'utf8mb4';
    const collationSel = document.getElementById('db-create-collation');
    if (!collationSel) return;
    const options = COLLATIONS[charset] || [charset + '_general_ci'];
    collationSel.innerHTML = options.map(c => `<option value="${c}">${c}</option>`).join('');
}

function updateCharsetVisibility() {
    const server_id = parseInt(document.getElementById('db-create-server-select')?.value);
    const selectedServer = registeredServers.find(s => s.id === server_id);
    const isPostgres = selectedServer && selectedServer.driver === 'postgres';
    const charsetGroup = document.getElementById('db-create-charset-group');
    const encodingGroup = document.getElementById('db-create-encoding-group');
    if (charsetGroup) charsetGroup.style.display = isPostgres ? 'none' : 'block';
    if (encodingGroup) encodingGroup.style.display = isPostgres ? 'block' : 'none';
}

// ==========================================
// 5. Multi-User per Database
// ==========================================

let activeManageDbId = null;

export function openManageDbUsersModal(dbId, dbName) {
    activeManageDbId = dbId;
    document.getElementById('manage-db-users-dbid').value = dbId;
    document.getElementById('manage-db-users-dbname').textContent = dbName;
    document.getElementById('new-db-user-name').value = '';
    document.getElementById('new-db-user-password').value = generateSecurePassword();
    document.getElementById('new-db-user-access').value = 'local';
    document.getElementById('new-db-user-privileges').value = 'ALL';
    loadDbUsers(dbId);
    document.getElementById('manage-db-users-modal').classList.add('active');
}

export function closeManageDbUsersModal() {
    document.getElementById('manage-db-users-modal').classList.remove('active');
    activeManageDbId = null;
}

export function loadDbUsers(dbId) {
    fetch(`/api/database/users?database_id=${encodeURIComponent(dbId)}`)
        .then(res => res.json())
        .then(res => {
            const tbody = document.getElementById('db-users-table-body');
            if (!tbody) return;
            if (!res.success || !res.data || res.data.length === 0) {
                tbody.innerHTML = '<tr><td colspan="4" style="text-align:center; padding:20px; color:var(--text-muted);">No additional users.</td></tr>';
                return;
            }
            tbody.innerHTML = res.data.map(u => `
                <tr>
                    <td style="font-family:var(--font-code); font-size:0.85rem; font-weight:600;">${escapeHtml(u.db_username)}</td>
                    <td><span class="badge ${u.access_type === 'local' ? 'badge-sleeping' : 'badge-running'}" style="font-size:0.7rem;">${u.access_type.toUpperCase()}</span></td>
                    <td style="font-size:0.8rem; color:var(--text-muted);">${escapeHtml(u.privileges)}</td>
                    <td style="text-align:right;">
                        <div style="display:flex; justify-content:flex-end; gap:6px;">
                            <button onclick="openChangeDbUserPasswordModal(${u.id}, '${escapeHtml(u.db_username)}')" style="background:rgba(255,255,255,0.08); border:none; color:#e4e4e7; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Change Password">
                                <i class="fa-solid fa-key"></i>
                            </button>
                            <button onclick="deleteDbUser(${u.id})" style="background:rgba(239,68,68,0.15); border:none; color:#f87171; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Delete User">
                                <i class="fa-solid fa-trash-can"></i>
                            </button>
                        </div>
                    </td>
                </tr>
            `).join('');
        })
        .catch(() => {
            const tbody = document.getElementById('db-users-table-body');
            if (tbody) tbody.innerHTML = '<tr><td colspan="4" style="text-align:center; color:var(--danger);">Error loading users</td></tr>';
        });
}

export function generateNewDbUserPassword() {
    document.getElementById('new-db-user-password').value = generateSecurePassword();
}

export function submitAddDbUser() {
    const database_id = parseInt(document.getElementById('manage-db-users-dbid').value);
    const db_username = document.getElementById('new-db-user-name').value.trim();
    const db_password = document.getElementById('new-db-user-password').value;
    const access_type = document.getElementById('new-db-user-access').value;
    const privileges  = document.getElementById('new-db-user-privileges').value;

    if (!db_username || !db_password) {
        showToast('error', 'Username dan password wajib diisi');
        return;
    }

    fetch('/api/database/users/add', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ database_id, db_username, db_password, access_type, privileges })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'User berhasil ditambahkan');
            document.getElementById('new-db-user-name').value = '';
            document.getElementById('new-db-user-password').value = generateSecurePassword();
            loadDbUsers(database_id);
        } else {
            showToast('error', res.message || 'Gagal menambahkan user');
        }
    })
    .catch(err => showToast('error', 'API error: ' + err.toString()));
}

export function deleteDbUser(userId) {
    if (!confirm('Hapus user database ini? User akan di-DROP dari server.')) return;

    fetch('/api/database/users/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ user_id: userId })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'User berhasil dihapus');
            if (activeManageDbId) loadDbUsers(activeManageDbId);
        } else {
            showToast('error', res.message || 'Gagal menghapus user');
        }
    })
    .catch(err => showToast('error', 'API error: ' + err.toString()));
}

export function openChangeDbUserPasswordModal(userId, username) {
    document.getElementById('change-dbuser-id').value = userId;
    document.getElementById('change-dbuser-name-label').textContent = username;
    document.getElementById('change-dbuser-password-field').value = generateSecurePassword();
    document.getElementById('change-dbuser-password-modal').classList.add('active');
}

export function closeChangeDbUserPasswordModal() {
    document.getElementById('change-dbuser-password-modal').classList.remove('active');
}

export function generateChangeDbUserPassword() {
    document.getElementById('change-dbuser-password-field').value = generateSecurePassword();
}

export function submitChangeDbUserPassword() {
    const user_id = parseInt(document.getElementById('change-dbuser-id').value);
    const password = document.getElementById('change-dbuser-password-field').value;

    if (!password) {
        showToast('error', 'Password tidak boleh kosong');
        return;
    }

    fetch('/api/database/users/change-password', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ user_id, password })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'Password user berhasil diperbarui');
            closeChangeDbUserPasswordModal();
            if (activeManageDbId) loadDbUsers(activeManageDbId);
        } else {
            showToast('error', res.message || 'Gagal update password');
        }
    })
    .catch(err => showToast('error', 'API error: ' + err.toString()));
}

// ==========================================
// 6. Install DB Engine via Container
// ==========================================

export function openInstallDbEngineModal() {
    document.getElementById('install-db-name').value = '';
    document.getElementById('install-db-root-password').value = generateSecurePassword();
    document.getElementById('install-db-port').value = '3306';
    document.getElementById('install-db-data-dir').value = '';
    document.getElementById('install-db-engine').value = 'mysql:8.4';
    onInstallDbEngineChange();
    document.getElementById('install-db-engine-modal').classList.add('active');
}

export function closeInstallDbEngineModal() {
    document.getElementById('install-db-engine-modal').classList.remove('active');
}

export function onInstallDbEngineChange() {
    const engine = document.getElementById('install-db-engine')?.value || 'mysql:8.4';
    const portField = document.getElementById('install-db-port');
    const nameField = document.getElementById('install-db-name');
    const dataDirField = document.getElementById('install-db-data-dir');
    const isPostgres = engine.startsWith('postgres');
    if (portField) portField.value = isPostgres ? '5432' : '3306';
    
    // Dynamically update Connection Pool label and default port
    const poolLabel = document.getElementById('install-db-pool-label');
    const poolPortField = document.getElementById('install-db-pool-port');
    if (poolLabel) {
        poolLabel.textContent = isPostgres ? 'Enable Connection Pool (PgBouncer)' : 'Enable Connection Pool (ProxySQL)';
    }
    if (poolPortField) {
        poolPortField.value = isPostgres ? '6432' : '6033';
    }

    // Suggest name and data dir/volume based on engine (only if field is empty)
    const shortName = engine.replace(':', '-').replace(/\./g, '').replace(/-/g, '_');
    if (nameField && !nameField.value) nameField.value = shortName;
    if (dataDirField && !dataDirField.value) {
        dataDirField.value = `${shortName}_data`;
    }
}

export function generateInstallDbRootPassword() {
    document.getElementById('install-db-root-password').value = generateSecurePassword();
}

export function toggleInstallDbPoolPortVisibility() {
    const enabled = document.getElementById('install-db-pool-enabled').checked;
    const group = document.getElementById('install-db-pool-port-group');
    if (group) {
        group.style.display = enabled ? 'block' : 'none';
    }
}

export function submitInstallDbEngine() {
    const engine = document.getElementById('install-db-engine').value;
    const name = document.getElementById('install-db-name').value.trim();
    const port = parseInt(document.getElementById('install-db-port').value) || 3306;
    const root_password = document.getElementById('install-db-root-password').value;
    const data_dir = document.getElementById('install-db-data-dir').value.trim();
    const mem_limit = document.getElementById('install-db-mem-limit').value;
    const cpus = document.getElementById('install-db-cpu-limit').value;
    const pool_enabled = document.getElementById('install-db-pool-enabled').checked ? 1 : 0;
    const pool_port = parseInt(document.getElementById('install-db-pool-port').value) || (port + 1000);

    if (!name || !root_password || !data_dir) {
        showToast('error', 'Semua field wajib diisi');
        return;
    }

    const btn = document.getElementById('install-db-submit-btn');
    if (btn) {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Deploying...';
    }

    showToast('info', 'Mendeploy container database... (ini mungkin membutuhkan beberapa menit)');

    fetch('/api/database/install-server', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ engine, name, port, root_password, data_dir, mem_limit, cpus, pool_enabled, pool_port })
    })
    .then(res => res.json())
    .then(res => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-rocket"></i> Deploy & Register';
        }
        if (res.success) {
            showToast('success', `Database engine ${engine} berhasil di-deploy dan didaftarkan!`);
            closeInstallDbEngineModal();
            loadDatabaseServers();
        } else {
            showToast('error', res.message || 'Gagal deploy container');
        }
    })
    .catch(err => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-rocket"></i> Deploy & Register';
        }
        showToast('error', 'API error: ' + err.toString());
    });
}

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
    
    let optionsHtml = '<option value="">Pilih Koneksi...</option>';
    
    // Add created user databases specifically
    userDatabases.forEach(db => {
        optionsHtml += `<option value="${escapeHtml(db.db_name)}">${escapeHtml(db.db_name)} (User DB - ${db.server_driver.toUpperCase()})</option>`;
    });

    // Add registered raw servers
    registeredServers.forEach(server => {
        optionsHtml += `<option value="${escapeHtml(server.name)}">${escapeHtml(server.name)} (Server Admin - ${server.driver.toUpperCase()})</option>`;
    });

    select.innerHTML = optionsHtml;
    
    // Maintain selection if still valid
    if (select.querySelector(`option[value="${currentVal}"]`)) {
        select.value = currentVal;
    }
}

// ==========================================
// 7. Database Backup & Restore
// ==========================================

export function loadDatabaseBackups() {
    const tbody = document.getElementById('db-backups-table-body');
    if (!tbody) return;

    fetch('/api/database/backups')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                // Update stats
                const stats = res.stats || { count: 0, total_size: 0, last_backup: '-' };
                
                const statCount = document.getElementById('backup-stat-count');
                const statSize = document.getElementById('backup-stat-size');
                const statLast = document.getElementById('backup-stat-last');
                
                if (statCount) statCount.innerText = stats.count;
                if (statSize) statSize.innerText = formatBytes(stats.total_size);
                if (statLast) {
                    if (stats.last_backup && stats.last_backup !== '-') {
                        statLast.innerText = new Date(stats.last_backup).toLocaleString();
                    } else {
                        statLast.innerText = '-';
                    }
                }

                if (res.data.length === 0) {
                    tbody.innerHTML = `
                        <tr>
                            <td colspan="7" style="text-align:center; padding:32px; color:var(--text-muted);">
                                Belum ada file backup database.
                            </td>
                        </tr>
                    `;
                    return;
                }

                tbody.innerHTML = res.data.map(item => {
                    let sizeStr = formatBytes(item.size);
                    let statusBadge = '';
                    if (item.status === 'success') {
                        statusBadge = '<span class="badge badge-running">Success</span>';
                    } else {
                        statusBadge = `<span class="badge badge-failed" title="${escapeHtml(item.status)}">Failed</span>`;
                    }

                    const dateStr = new Date(item.created_at).toLocaleString();

                    return `
                        <tr>
                            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(item.database_name)}</td>
                            <td>${escapeHtml(item.server_name)}</td>
                            <td style="font-family:var(--font-code); font-size:0.8rem;">${escapeHtml(item.filename)}</td>
                            <td>${sizeStr}</td>
                            <td>${statusBadge}</td>
                            <td>${dateStr}</td>
                            <td style="text-align:right;">
                                <div style="display:flex; justify-content:flex-end; gap:6px;">
                                    ${item.status === 'success' ? `
                                        <button onclick="downloadDatabaseBackup('${escapeHtml(item.filename)}')" style="background:rgba(59,130,246,0.15); border:none; color:#60a5fa; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Download Backup">
                                            <i class="fa-solid fa-download"></i>
                                        </button>
                                        <button onclick="triggerRestoreDatabaseBackup(${item.id}, '${escapeHtml(item.database_name)}', '${escapeHtml(item.filename)}', '${item.created_at}', ${item.size || 0})" style="background:rgba(16,185,129,0.15); border:none; color:#34d399; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Restore Backup">
                                            <i class="fa-solid fa-rotate-left"></i>
                                        </button>
                                    ` : ''}
                                    <button onclick="deleteDatabaseBackup(${item.id}, '${escapeHtml(item.filename)}')" style="background:rgba(239,68,68,0.15); border:none; color:#f87171; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Delete Backup">
                                        <i class="fa-solid fa-trash-can"></i>
                                    </button>
                                </div>
                            </td>
                        </tr>
                    `;
                }).join('');
            }
        })
        .catch(err => {
            console.error('Failed to load database backups:', err);
            tbody.innerHTML = `<tr><td colspan="7" style="text-align:center; color:var(--danger); padding:32px;">Gagal memuat daftar backup: ${escapeHtml(err.toString())}</td></tr>`;
        });
}

function formatBytes(bytes, decimals = 2) {
    if (!bytes || bytes === 0) return '0 Bytes';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

export function openCreateBackupModal() {
    const select = document.getElementById('db-backup-select');
    if (!select) return;

    let optionsHtml = '';
    userDatabases.forEach(db => {
        optionsHtml += `<option value="${escapeHtml(db.db_name)}">${escapeHtml(db.db_name)} (${escapeHtml(db.server_name)} - ${db.server_driver.toUpperCase()})</option>`;
    });
    select.innerHTML = optionsHtml;

    document.getElementById('create-db-backup-modal').classList.add('active');
}

export function closeCreateBackupModal() {
    document.getElementById('create-db-backup-modal').classList.remove('active');
}

export function submitCreateBackup() {
    const database_name = document.getElementById('db-backup-select').value;
    if (!database_name) return;

    const btn = document.getElementById('create-db-backup-submit-btn');
    if (btn) {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Backing up...';
    }

    showToast('info', 'Sedang memproses backup database...');

    fetch('/api/database/backups/create', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ database_name })
    })
    .then(res => res.json())
    .then(res => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-save"></i> Jalankan Backup';
        }
        if (res.success) {
            showToast('success', res.message || 'Backup berhasil dibuat');
            closeCreateBackupModal();
            loadDatabaseBackups();
        } else {
            showToast('error', res.message || 'Gagal membuat backup');
        }
    })
    .catch(err => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-save"></i> Jalankan Backup';
        }
        showToast('error', 'API Error: ' + err.toString());
    });
}

export function triggerRestoreDatabaseBackup(id, dbName, filename, createdAt, sizeBytes) {
    // Populate modal detail fields
    document.getElementById('restore-backup-id-val').value = id;
    document.getElementById('restore-detail-db').textContent = dbName;
    document.getElementById('restore-detail-label').textContent = filename || '-';
    document.getElementById('restore-detail-date').textContent = createdAt ? new Date(createdAt).toLocaleString() : '-';
    document.getElementById('restore-detail-size').textContent = sizeBytes ? formatBytes(sizeBytes) : '-';

    // Hide progress banner
    const prog = document.getElementById('restore-progress-banner');
    if (prog) prog.style.display = 'none';

    // Re-enable restore button
    const btn = document.getElementById('btn-restore-submit');
    if (btn) {
        btn.disabled = false;
        btn.innerHTML = '<i class="fa-solid fa-rotate-left"></i> Yes, Restore Now';
    }

    document.getElementById('confirm-restore-db-backup-modal').classList.add('active');
}

export function closeConfirmRestoreModal() {
    document.getElementById('confirm-restore-db-backup-modal').classList.remove('active');
}

export function confirmRestoreDatabaseBackup() {
    const id = document.getElementById('restore-backup-id-val').value;
    const dbName = document.getElementById('restore-detail-db').textContent;
    if (!id) return;

    const btn = document.getElementById('btn-restore-submit');
    if (btn) {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-circle-notch fa-spin"></i> Restoring...';
    }

    const prog = document.getElementById('restore-progress-banner');
    if (prog) prog.style.display = 'block';

    fetch('/api/database/backups/restore', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id: parseInt(id, 10) })
    })
    .then(res => res.json())
    .then(res => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-rotate-left"></i> Yes, Restore Now';
        }
        if (prog) prog.style.display = 'none';

        if (res.success) {
            closeConfirmRestoreModal();
            showToast('success', res.message || 'Database berhasil di-restore');
            if (dbName === 'default') {
                showToast('info', 'Memuat ulang halaman dalam 2 detik...');
                setTimeout(() => { window.location.reload(); }, 2000);
            } else {
                loadDatabaseBackups();
            }
        } else {
            showToast('error', res.message || 'Gagal merestore database');
        }
    })
    .catch(err => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-rotate-left"></i> Yes, Restore Now';
        }
        if (prog) prog.style.display = 'none';
        showToast('error', 'API Error: ' + err.toString());
    });
}

export function deleteDatabaseBackup(id, filename) {
    if (!confirm(`Apakah Anda yakin ingin menghapus file backup "${filename}"? File akan dihapus permanen dari disk.`)) {
        return;
    }

    fetch('/api/database/backups/delete', {
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
            showToast('success', res.message || 'File backup berhasil dihapus');
            loadDatabaseBackups();
        } else {
            showToast('error', res.message || 'Gagal menghapus backup');
        }
    })
    .catch(err => {
        showToast('error', 'API Error: ' + err.toString());
    });
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
window.runSqlQuery = runSqlQuery;

// Charset & Collation
window.onDbCharsetChange = onDbCharsetChange;

// Multi-user management
window.openManageDbUsersModal = openManageDbUsersModal;
window.closeManageDbUsersModal = closeManageDbUsersModal;
window.generateNewDbUserPassword = generateNewDbUserPassword;
window.submitAddDbUser = submitAddDbUser;
window.deleteDbUser = deleteDbUser;
window.openChangeDbUserPasswordModal = openChangeDbUserPasswordModal;
window.closeChangeDbUserPasswordModal = closeChangeDbUserPasswordModal;
window.generateChangeDbUserPassword = generateChangeDbUserPassword;
window.submitChangeDbUserPassword = submitChangeDbUserPassword;

// Install DB Engine
window.openInstallDbEngineModal = openInstallDbEngineModal;
window.closeInstallDbEngineModal = closeInstallDbEngineModal;
window.onInstallDbEngineChange = onInstallDbEngineChange;
window.generateInstallDbRootPassword = generateInstallDbRootPassword;
window.submitInstallDbEngine = submitInstallDbEngine;

// Database Backup & Restore
window.loadDatabaseBackups = loadDatabaseBackups;
window.openCreateBackupModal = openCreateBackupModal;
window.closeCreateBackupModal = closeCreateBackupModal;
window.submitCreateBackup = submitCreateBackup;
window.triggerRestoreDatabaseBackup = triggerRestoreDatabaseBackup;
window.closeConfirmRestoreModal = closeConfirmRestoreModal;
window.confirmRestoreDatabaseBackup = confirmRestoreDatabaseBackup;
window.deleteDatabaseBackup = deleteDatabaseBackup;

// New competitive features
export function toggleDatabaseRemoteAccess(serverId, makeRemote) {
    showToast('info', 'Mengubah konfigurasi akses remote database...');
    fetch('/api/database/servers/toggle-remote', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ id: serverId, is_remote: makeRemote ? 1 : 0 })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            loadDatabaseServers();
        } else {
            showToast('error', res.message || 'Gagal mengubah konfigurasi remote');
        }
    })
    .catch(err => showToast('error', 'API error: ' + err.toString()));
}

export function toggleDatabasePool(serverId, enablePool) {
    showToast('info', 'Mengubah konfigurasi connection pool database...');
    fetch('/api/database/servers/toggle-pool', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ id: serverId, pool_enabled: enablePool ? 1 : 0 })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            loadDatabaseServers();
        } else {
            showToast('error', res.message || 'Gagal mengubah konfigurasi connection pool');
        }
    })
    .catch(err => showToast('error', 'API error: ' + err.toString()));
}

export function openDatabaseLogsModal(serverName) {
    const content = document.getElementById('db-logs-content');
    if (content) content.textContent = 'Loading log kontainer...';
    document.getElementById('db-logs-modal').classList.add('active');

    fetch(`/api/database/servers/logs?name=${encodeURIComponent(serverName)}`)
        .then(res => res.json())
        .then(res => {
            if (res.success && content) {
                content.textContent = res.logs;
            } else if (content) {
                content.textContent = 'Gagal memuat log kontainer.';
            }
        })
        .catch(err => {
            if (content) content.textContent = 'Error: ' + err.toString();
        });
}

export function closeDatabaseLogsModal() {
    document.getElementById('db-logs-modal').classList.remove('active');
}

export function downloadDatabaseBackup(filename) {
    const path = `/var/lib/zenopanel/backups/db/${filename}`;
    window.location.href = `/api/files/download?path=${encodeURIComponent(path)}`;
}

// ==========================================
// 8. Database Server Configuration Tuning
// ==========================================

let activeConfigServerId = null;

export function openDatabaseConfigModal(serverId, name, driver) {
    activeConfigServerId = serverId;
    document.getElementById('db-config-server-id').value = serverId;
    document.getElementById('db-config-server-name').innerText = name;
    document.getElementById('db-config-server-driver').value = driver;

    // Reset tabs
    switchDbConfigTab('visual');

    // Update labels/hints based on database driver
    const isPostgres = driver === 'postgres';
    document.getElementById('db-config-buffer-label').innerText = isPostgres ? 'Shared Buffers' : 'Buffer Pool Size';
    document.getElementById('db-config-packet-label').innerText = isPostgres ? 'Work Memory' : 'Max Allowed Packet';

    // Populate with defaults / placeholders
    document.getElementById('db-config-max-connections').value = isPostgres ? '100' : '150';
    document.getElementById('db-config-buffer-pool').value = isPostgres ? '128MB' : '128M';
    document.getElementById('db-config-max-packet').value = isPostgres ? '4MB' : '16M';
    setDbConfigContent('');

    // Show loading indicators for live values
    document.getElementById('db-config-live-connections').innerText = '...';
    document.getElementById('db-config-live-buffer').innerText = '...';
    document.getElementById('db-config-live-packet').innerText = '...';

    document.getElementById('db-server-config-modal').classList.add('active');

    // Fetch config values from API
    fetch(`/api/database/servers/config?id=${serverId}`)
        .then(res => res.json())
        .then(res => {
            if (res.success) {
                document.getElementById('db-config-live-connections').innerText = res.live_max_connections;
                document.getElementById('db-config-live-buffer').innerText = res.live_buffer_pool;
                document.getElementById('db-config-live-packet').innerText = res.live_packet;

                if (res.live_max_connections && res.live_max_connections !== 'Unknown') {
                    document.getElementById('db-config-max-connections').value = res.live_max_connections;
                }
                if (res.live_buffer_pool && res.live_buffer_pool !== 'Unknown') {
                    document.getElementById('db-config-buffer-pool').value = res.live_buffer_pool;
                }
                if (res.live_packet && res.live_packet !== 'Unknown') {
                    document.getElementById('db-config-max-packet').value = res.live_packet;
                }

                setDbConfigContent(res.config || '');
            } else {
                showToast('error', 'Gagal memuat konfigurasi server');
            }
        })
        .catch(err => showToast('error', 'Gagal memuat konfigurasi: ' + err.toString()));
}

export function closeDatabaseConfigModal() {
    document.getElementById('db-server-config-modal').classList.remove('active');
    activeConfigServerId = null;
    if (dbConfigMonacoInstance) {
        dbConfigMonacoInstance.dispose();
        dbConfigMonacoInstance = null;
    }
}

export function switchDbConfigTab(tabName) {
    const visualTab = document.getElementById('db-config-tab-visual');
    const rawTab = document.getElementById('db-config-tab-raw');
    const visualBtn = document.getElementById('db-config-tab-visual-btn');
    const rawBtn = document.getElementById('db-config-tab-raw-btn');

    if (tabName === 'visual') {
        if (visualTab) visualTab.style.display = 'block';
        if (rawTab) rawTab.style.display = 'none';
        if (visualBtn) visualBtn.classList.add('active');
        if (rawBtn) rawBtn.classList.remove('active');
    } else {
        if (visualTab) visualTab.style.display = 'none';
        if (rawTab) rawTab.style.display = 'block';
        if (visualBtn) visualBtn.classList.remove('active');
        if (rawBtn) rawBtn.classList.add('active');
    }
}

export function submitDatabaseConfig() {
    const id = activeConfigServerId;
    if (!id) return;

    const max_connections = document.getElementById('db-config-max-connections').value.trim();
    const buffer_pool_size = document.getElementById('db-config-buffer-pool').value.trim();
    const max_allowed_packet = document.getElementById('db-config-max-packet').value.trim();
    
    const rawBtn = document.getElementById('db-config-tab-raw-btn');
    const isRawActive = rawBtn && rawBtn.classList.contains('active');
    const config = isRawActive ? getDbConfigContent() : '';

    const btn = document.getElementById('db-config-save-btn');
    if (btn) {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Saving...';
    }

    showToast('info', 'Sedang menyimpan konfigurasi dan merestart database...');

    fetch('/api/database/servers/config/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ id, config, max_connections, buffer_pool_size, max_allowed_packet })
    })
    .then(res => res.json())
    .then(res => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-save"></i> Save &amp; Restart';
        }
        if (res.success) {
            showToast('success', res.message);
            closeDatabaseConfigModal();
            loadDatabaseServers();
        } else {
            showToast('error', res.message || 'Gagal menyimpan konfigurasi');
        }
    })
    .catch(err => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-save"></i> Save &amp; Restart';
        }
        showToast('error', 'API error: ' + err.toString());
    });
}

// ==========================================
// 9. Database Maintenance Actions
// ==========================================

export function openDatabaseMaintenanceModal(dbName, serverName, driver) {
    document.getElementById('db-maint-db-name').value = dbName;
    document.getElementById('db-maint-server-name').value = serverName;
    document.getElementById('db-maint-driver').value = driver;

    document.getElementById('db-maint-label-dbname').innerText = dbName;
    document.getElementById('db-maint-label-driver').innerText = driver;
    document.getElementById('db-maint-output').innerText = 'Pilih salah satu operasi di atas untuk memulai perawatan database.';

    const repairBtn = document.getElementById('maint-btn-repair');
    if (repairBtn) {
        repairBtn.style.display = driver === 'postgres' ? 'none' : 'flex';
    }

    document.getElementById('db-maintenance-modal').classList.add('active');
}

export function closeDatabaseMaintenanceModal() {
    document.getElementById('db-maintenance-modal').classList.remove('active');
}

export function runDatabaseMaintenanceAction(action) {
    const dbName = document.getElementById('db-maint-db-name').value;
    const serverName = document.getElementById('db-maint-server-name').value;
    const driver = document.getElementById('db-maint-driver').value;
    const outputDiv = document.getElementById('db-maint-output');

    if (outputDiv) {
        outputDiv.innerHTML = `<span style="color:var(--accent-primary);"><i class="fa-solid fa-spinner fa-spin"></i> Running ${action} database...</span>`;
    }

    fetch('/api/database/maintenance', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ server_name: serverName, db_name: dbName, driver, action })
    })
    .then(res => res.json())
    .then(res => {
        if (outputDiv) {
            if (res.success) {
                outputDiv.innerHTML = `<span style="color:#10b981; font-weight:600;"><i class="fa-solid fa-circle-check"></i> Sukses!</span>\n\n${res.message}`;
                fetch(`/api/database/size?server_name=${encodeURIComponent(serverName)}&db_name=${encodeURIComponent(dbName)}&driver=${encodeURIComponent(driver)}`)
                    .then(r => r.json())
                    .then(r => {
                        const el = document.getElementById(`db-size-${dbName}`);
                        if (el && r.success) el.innerText = formatBytes(r.size);
                    });
            } else {
                outputDiv.innerHTML = `<span style="color:#ef4444; font-weight:600;"><i class="fa-solid fa-circle-xmark"></i> Gagal!</span>\n\n${res.message}`;
            }
        }
    })
    .catch(err => {
        if (outputDiv) {
            outputDiv.innerHTML = `<span style="color:#ef4444; font-weight:600;"><i class="fa-solid fa-triangle-exclamation"></i> API Error</span>\n\n${err.toString()}`;
        }
    });
}

// Uptime Formatter Helper
function formatUptime(seconds) {
    if (!seconds || seconds === '0' || seconds === 0) return '0s';
    const s = parseInt(seconds, 10);
    const d = Math.floor(s / 86400);
    const h = Math.floor((s % 86400) / 3600);
    const m = Math.floor((s % 3600) / 60);
    
    let res = '';
    if (d > 0) res += `${d}d `;
    if (h > 0) res += `${h}h `;
    if (m > 0) res += `${m}m `;
    if (res === '') res = `${s}s`;
    return res.trim();
}

window.toggleDatabaseRemoteAccess = toggleDatabaseRemoteAccess;
window.toggleDatabasePool = toggleDatabasePool;
window.toggleInstallDbPoolPortVisibility = toggleInstallDbPoolPortVisibility;
window.openDatabaseLogsModal = openDatabaseLogsModal;
window.closeDatabaseLogsModal = closeDatabaseLogsModal;
window.downloadDatabaseBackup = downloadDatabaseBackup;

window.openDatabaseConfigModal = openDatabaseConfigModal;
window.closeDatabaseConfigModal = closeDatabaseConfigModal;
window.switchDbConfigTab = switchDbConfigTab;
window.submitDatabaseConfig = submitDatabaseConfig;
window.openDatabaseMaintenanceModal = openDatabaseMaintenanceModal;
window.closeDatabaseMaintenanceModal = closeDatabaseMaintenanceModal;
window.runDatabaseMaintenanceAction = runDatabaseMaintenanceAction;

// ==========================================
// 10. Database Scheduled Backups Management
// ==========================================

export function loadDatabaseBackupSchedules() {
    const tbody = document.getElementById('db-backup-schedules-table-body');
    if (!tbody) return;

    fetch('/api/database/backup-schedules')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                if (res.data.length === 0) {
                    tbody.innerHTML = `
                        <tr>
                            <td colspan="5" style="text-align:center; padding:20px; color:var(--text-muted);">No auto-backup schedules configured.</td>
                        </tr>
                    `;
                    return;
                }

                tbody.innerHTML = res.data.map(sched => {
                    const lastRun = sched.last_run ? sched.last_run.replace('T', ' ').substring(0, 19) : '-';
                    return `
                        <tr>
                            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(sched.database_name)}</td>
                            <td style="text-transform:capitalize;">${escapeHtml(sched.schedule)}</td>
                            <td>${sched.retention} files</td>
                            <td style="font-size:0.85rem; color:var(--text-muted);">${escapeHtml(lastRun)}</td>
                            <td style="text-align:right;">
                                <button onclick="deleteDatabaseBackupSchedule(${sched.id})" style="background:rgba(239,68,68,0.15); border:none; color:#f87171; padding:6px 10px; border-radius:6px; cursor:pointer; transition:all 0.2s;" title="Delete Schedule">
                                    <i class="fa-solid fa-trash-can"></i>
                                </button>
                            </td>
                        </tr>
                    `;
                }).join('');
            } else {
                tbody.innerHTML = `<tr><td colspan="5" style="text-align:center; color:var(--danger); padding:20px;">Gagal memuat jadwal: ${escapeHtml(res.message || 'Error')}</td></tr>`;
            }
        })
        .catch(err => {
            tbody.innerHTML = `<tr><td colspan="5" style="text-align:center; color:var(--danger); padding:20px;">Gagal memuat jadwal: ${escapeHtml(err.toString())}</td></tr>`;
        });
}

export function openCreateBackupScheduleModal() {
    const select = document.getElementById('db-backup-sched-database');
    if (!select) return;

    // Populate dropdown with user databases (excluding SQLite)
    let optionsHtml = '';
    userDatabases.forEach(db => {
        optionsHtml += `<option value="${escapeHtml(db.db_name)}">${escapeHtml(db.db_name)} (${escapeHtml(db.server_name)})</option>`;
    });

    select.innerHTML = optionsHtml;
    document.getElementById('db-backup-sched-retention').value = '7';
    document.getElementById('create-db-backup-schedule-modal').classList.add('active');
}

export function closeCreateBackupScheduleModal() {
    document.getElementById('create-db-backup-schedule-modal').classList.remove('active');
}

export function submitCreateBackupSchedule() {
    const database_name = document.getElementById('db-backup-sched-database').value;
    const schedule = document.getElementById('db-backup-sched-period').value;
    const retention = parseInt(document.getElementById('db-backup-sched-retention').value, 10);

    if (!database_name) {
        showToast('error', 'Silakan pilih database terlebih dahulu.');
        return;
    }
    if (isNaN(retention) || retention < 1) {
        showToast('error', 'Batas retensi tidak valid.');
        return;
    }

    fetch('/api/database/backup-schedules/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ database_name, schedule, retention })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            closeCreateBackupScheduleModal();
            loadDatabaseBackupSchedules();
        } else {
            showToast('error', res.message || 'Gagal menyimpan jadwal backup.');
        }
    })
    .catch(err => {
        showToast('error', 'API error: ' + err.toString());
    });
}

export function deleteDatabaseBackupSchedule(id) {
    if (!confirm('Apakah Anda yakin ingin menghapus jadwal backup ini?')) return;

    fetch('/api/database/backup-schedules/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
        body: JSON.stringify({ id })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            loadDatabaseBackupSchedules();
        } else {
            showToast('error', res.message || 'Gagal menghapus jadwal backup.');
        }
    })
    .catch(err => {
        showToast('error', 'API error: ' + err.toString());
    });
}

window.loadDatabaseBackupSchedules = loadDatabaseBackupSchedules;
window.openCreateBackupScheduleModal = openCreateBackupScheduleModal;
window.closeCreateBackupScheduleModal = closeCreateBackupScheduleModal;
window.submitCreateBackupSchedule = submitCreateBackupSchedule;
window.deleteDatabaseBackupSchedule = deleteDatabaseBackupSchedule;

export function openImportDatabaseModal(dbName) {
    const labelGroup = document.getElementById('import-db-name-label-group');
    const selectGroup = document.getElementById('import-db-name-select-group');
    const valInput = document.getElementById('import-db-name-val');
    const label = document.getElementById('import-db-name-label');
    const select = document.getElementById('import-db-target-select');
    
    // Reset file input
    document.getElementById('import-db-file-input').value = '';
    
    // Hide progress banner
    const prog = document.getElementById('import-progress-banner');
    if (prog) prog.style.display = 'none';
    
    // Re-enable import button
    const btn = document.getElementById('import-db-submit-btn');
    if (btn) {
        btn.disabled = false;
        btn.innerHTML = '<i class="fa-solid fa-upload"></i> Upload &amp; Import';
    }

    if (dbName) {
        // Pre-fill database name
        valInput.value = dbName;
        label.textContent = dbName;
        labelGroup.style.display = 'block';
        selectGroup.style.display = 'none';
    } else {
        // Show database selector
        valInput.value = '';
        labelGroup.style.display = 'none';
        selectGroup.style.display = 'block';
        
        // Populate dropdown
        if (select) {
            select.innerHTML = userDatabases.map(db => `
                <option value="${escapeHtml(db.db_name)}">${escapeHtml(db.db_name)} (${escapeHtml(db.server_name)} - ${escapeHtml(db.server_driver.toUpperCase())})</option>
            `).join('');
        }
    }

    document.getElementById('import-db-modal').classList.add('active');
}

export function closeImportDatabaseModal() {
    document.getElementById('import-db-modal').classList.remove('active');
}

export function submitImportDatabase() {
    const valInput = document.getElementById('import-db-name-val').value;
    const selectVal = document.getElementById('import-db-target-select')?.value;
    const dbName = valInput || selectVal;
    
    if (!dbName) {
        showToast('error', 'Silakan pilih database target.');
        return;
    }

    const fileInput = document.getElementById('import-db-file-input');
    const file = fileInput ? fileInput.files[0] : null;

    if (!file) {
        showToast('error', 'Silakan pilih file SQL atau database untuk diunggah.');
        return;
    }

    const btn = document.getElementById('import-db-submit-btn');
    if (btn) {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-circle-notch fa-spin"></i> Importing...';
    }

    const prog = document.getElementById('import-progress-banner');
    if (prog) prog.style.display = 'block';

    const formData = new FormData();
    formData.append('file', file);

    fetch(`/api/database/import?database_name=${encodeURIComponent(dbName)}&filename=${encodeURIComponent(file.name)}`, {
        method: 'POST',
        headers: {
            'X-CSRF-Token': getCSRFToken()
        },
        body: formData
    })
    .then(res => res.json())
    .then(res => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-upload"></i> Upload &amp; Import';
        }
        if (prog) prog.style.display = 'none';

        if (res.success) {
            showToast('success', res.message || 'Database berhasil di-import');
            closeImportDatabaseModal();
            loadUserDatabases();
        } else {
            showToast('error', res.message || 'Gagal meng-import database');
        }
    })
    .catch(err => {
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-upload"></i> Upload &amp; Import';
        }
        if (prog) prog.style.display = 'none';
        showToast('error', 'API Error: ' + err.toString());
    });
}

window.openImportDatabaseModal = openImportDatabaseModal;
window.closeImportDatabaseModal = closeImportDatabaseModal;
window.submitImportDatabase = submitImportDatabase;


