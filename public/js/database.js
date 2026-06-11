import { getCSRFToken, escapeHtml } from './utils.js';
import { showToast } from './toast.js';

export function loadDatabaseTables() {
    fetch('/api/database/tables')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const list = document.getElementById('db-tables-list');
                if (!list) return;
                list.innerHTML = '';
                
                const badge = document.getElementById('db-driver-badge');
                if (badge) badge.innerText = res.driver.toUpperCase();

                if (res.data.length === 0) {
                    list.innerHTML = '<div style="padding:16px; text-align:center; color:var(--text-muted); font-size:0.8rem;">No tables found.</div>';
                    return;
                }

                res.data.forEach(item => {
                    const tableName = item.name || Object.values(item)[0];
                    const div = document.createElement('div');
                    div.className = 'db-table-item';
                    div.innerHTML = `<i class="fa-solid fa-table"></i> <span>${tableName}</span>`;
                    div.onclick = () => {
                        const q = `SELECT * FROM ${tableName} LIMIT 15;`;
                        const sqlQueryInput = document.getElementById('sql-query-input');
                        const sqlIsSelect = document.getElementById('sql-is-select');
                        if (sqlQueryInput) sqlQueryInput.value = q;
                        if (sqlIsSelect) sqlIsSelect.checked = true;
                    };
                    list.appendChild(div);
                });
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

    fetch('/api/database/query', {
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
        <table style="width:100%">
            <thead>
                <tr>
                    ${keys.map(k => `<th>${k}</th>`).join('')}
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
