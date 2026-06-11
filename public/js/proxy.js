import { getCSRFToken, escapeHtml } from './utils.js';
import { showToast } from './toast.js';
import { managedState } from './managed.js';

export let allProxyRules = [];

function sanitizeHost(input) {
    if (!input) return '';
    let host = input.trim();
    if (host === '*') return host;
    if (host.includes('://')) {
        host = host.split('://')[1];
    }
    if (host.includes('/')) {
        host = host.split('/')[0];
    }
    return host;
}

export function loadProxyRules() {
    const tbody = document.getElementById('proxy-table-body');
    if (tbody) {
        tbody.innerHTML = '<tr><td colspan="8" style="text-align:center; padding:30px; color:var(--text-muted);"><i class="fa-solid fa-spinner fa-spin"></i> Loading proxy rules...</td></tr>';
    }
    
    // Pre-fetch managed processes list so dropdown autofill is immediately ready
    fetch('/api/managed/list')
        .then(res => res.json())
        .then(res => {
            if (res.data) {
                managedState.allManagedProcesses = res.data;
            }
        })
        .catch(err => console.error('Failed to pre-fetch managed processes:', err));

    fetch('/api/proxy/list')
        .then(res => res.json())
        .then(res => {
            if (res.data) {
                allProxyRules = res.data;
                renderProxyRules(res.data);
            } else {
                if (tbody) {
                    tbody.innerHTML = '<tr><td colspan="8" style="text-align:center; padding:30px; color:var(--danger);">Failed to load proxy rules.</td></tr>';
                }
            }
        })
        .catch(err => {
            console.error('Failed to load proxy rules:', err);
            if (tbody) {
                tbody.innerHTML = `
                    <tr>
                        <td colspan="8" style="text-align:center; padding:30px; color:var(--danger);">
                            Failed to load proxy rules: ${escapeHtml(err.toString())}
                        </td>
                    </tr>
                `;
            }
        });
}

export function renderProxyRules(rules) {
    const tbody = document.getElementById('proxy-table-body');
    if (!tbody) return;
    tbody.innerHTML = '';

    if (rules.length === 0) {
        tbody.innerHTML = `
            <tr>
                <td colspan="8" style="text-align:center; padding:30px; color:var(--text-muted);">
                    <i class="fa-solid fa-network-wired" style="font-size:2rem; margin-bottom:10px; display:block; opacity:0.3;"></i>
                    No proxy rules registered yet.
                </td>
            </tr>
        `;
        return;
    }

    rules.forEach(rule => {
        const tr = document.createElement('tr');
        
        const isEnabled = rule.enabled;
        const statusToggle = `
            <div style="display:flex; align-items:center; gap:8px;">
                <label class="switch" style="position:relative; display:inline-block; width:36px; height:20px;">
                    <input type="checkbox" ${isEnabled ? 'checked' : ''} onchange="toggleProxy('${rule.id}', this.checked)" style="opacity:0; width:0; height:0;">
                    <span class="slider" style="position:absolute; cursor:pointer; top:0; left:0; right:0; bottom:0; transition:.3s; border-radius:20px; border:1px solid var(--card-border);"></span>
                </label>
                <span style="font-size:0.75rem; color:${isEnabled ? 'var(--success)' : 'var(--text-muted)'}; font-weight:500;">
                    ${isEnabled ? 'Active' : 'Disabled'}
                </span>
            </div>
        `;

        // SSL Badge rendering
        let sslBadge = '';
        if (rule.ssl_enabled) {
            if (rule.ssl_status === 'active_letsencrypt') {
                sslBadge = `<span class="badge" style="background:rgba(16,185,129,0.12); color:var(--success); border:1px solid rgba(16,185,129,0.15);" title="Active (Let's Encrypt)"><i class="fa-solid fa-lock" style="margin-right:4px;"></i> Let's Encrypt</span>`;
            } else if (rule.ssl_status === 'active_self_signed') {
                sslBadge = `<span class="badge" style="background:rgba(245,158,11,0.12); color:var(--warning); border:1px solid rgba(245,158,11,0.15);" title="Active (Self-Signed)"><i class="fa-solid fa-lock-open" style="margin-right:4px;"></i> Self-Signed</span>`;
            } else if (rule.ssl_status === 'pending') {
                sslBadge = `<span class="badge" style="background:rgba(255,255,255,0.05); color:var(--text-muted); border:1px solid var(--card-border);" title="Issuing certificate..."><i class="fa-solid fa-spinner fa-spin" style="margin-right:4px;"></i> Pending</span>`;
            } else if (rule.ssl_status === 'failed') {
                sslBadge = `<span class="badge" style="background:rgba(239,68,68,0.12); color:#ef4444; border:1px solid rgba(239,68,68,0.15);" title="Let's Encrypt challenge failed. Fallback to self-signed certificate."><i class="fa-solid fa-circle-exclamation" style="margin-right:4px;"></i> Failed</span>`;
            } else {
                sslBadge = `<span class="badge" style="background:rgba(255,255,255,0.05); color:var(--text-muted); border:1px solid var(--card-border);"><i class="fa-solid fa-lock-open" style="margin-right:4px;"></i> Enabled</span>`;
            }
        } else {
            sslBadge = `<span class="badge" style="background:rgba(255,255,255,0.02); color:var(--text-muted); border:1px solid transparent; opacity:0.5;"><i class="fa-solid fa-unlock" style="margin-right:4px;"></i> Disabled</span>`;
        }

        tr.innerHTML = `
            <td style="font-weight:600; color:var(--text-main);">${escapeHtml(rule.name)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">
                ${rule.domain ? escapeHtml(rule.domain) : '*'}
                ${rule.alternative_domain ? `<div style="font-size:0.75rem; color:var(--text-muted); margin-top:2px;">Alt: ${escapeHtml(rule.alternative_domain)}</div>` : ''}
            </td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${escapeHtml(rule.path)}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem; color:var(--accent-primary);">${escapeHtml(rule.target)}</td>
            <td>
                <span class="badge" style="background:${rule.strip_path ? 'rgba(59,130,246,0.12)' : 'rgba(255,255,255,0.05)'}; color:${rule.strip_path ? 'var(--accent-primary)' : 'var(--text-muted)'}; border:1px solid ${rule.strip_path ? 'rgba(59,130,246,0.1)' : 'var(--card-border)'};">
                    ${rule.strip_path ? 'Yes' : 'No'}
                </span>
            </td>
            <td>${sslBadge}</td>
            <td>${statusToggle}</td>
            <td style="text-align:right;">
                <div style="display:inline-flex; gap:8px;">
                    <button class="btn-action" onclick="openEditProxyModal('${rule.id}')" title="Edit Rule" style="padding:4px 8px; font-size:0.75rem; color:var(--warning); border-color:rgba(245,158,11,0.2); background:rgba(245,158,11,0.05);">
                        <i class="fa-solid fa-pen-to-square"></i> Edit
                    </button>
                    <button class="btn-action" onclick="deleteProxy('${rule.id}')" title="Delete Rule" style="padding:4px 8px; font-size:0.75rem; color:#ef4444; border-color:transparent; background:transparent;">
                        <i class="fa-solid fa-trash-can"></i>
                    </button>
                </div>
            </td>
        `;
        tbody.appendChild(tr);
    });
}

function extractPortFromProcess(proc) {
    if (!proc) return null;
    
    // 0. Check if backend detected open/listening ports
    if (proc.ports && Array.isArray(proc.ports) && proc.ports.length > 0) {
        return proc.ports[0];
    }
    
    // 1. Try to find in env
    if (proc.env) {
        // env might be a JSON object or string
        let envObj = proc.env;
        if (typeof envObj === 'string') {
            try {
                envObj = JSON.parse(envObj);
            } catch (e) {
                envObj = {};
            }
        }
        const portKeys = ['PORT', 'APP_PORT', 'port', 'Port'];
        for (const key of portKeys) {
            if (envObj[key]) {
                const p = parseInt(envObj[key].toString().replace(/^\D+/g, ''), 10);
                if (!isNaN(p) && p > 0 && p <= 65535) {
                    return p;
                }
            }
        }
    }
    
    // 2. Try to find in command using common patterns
    const cmd = proc.command || '';
    
    // Try to match --port 8080 or -p 8080 or --port=8080 or -port=8080
    const portFlagRegex = /(?:--port|-p|-port|--addr|--address)(?:\s+|=)(\d+)/i;
    const flagMatch = cmd.match(portFlagRegex);
    if (flagMatch && flagMatch[1]) {
        const p = parseInt(flagMatch[1], 10);
        if (!isNaN(p) && p > 0 && p <= 65535) {
            return p;
        }
    }

    // Try to match PORT=8080 or port=8080
    const portEnvInCmdRegex = /(?:PORT|port)\s*=\s*(\d+)/;
    const envCmdMatch = cmd.match(portEnvInCmdRegex);
    if (envCmdMatch && envCmdMatch[1]) {
        const p = parseInt(envCmdMatch[1], 10);
        if (!isNaN(p) && p > 0 && p <= 65535) {
            return p;
        }
    }
    
    // Try to match :8080 or localhost:8080
    const colonPortRegex = /(?::)(\d{4,5})\b/;
    const colonMatch = cmd.match(colonPortRegex);
    if (colonMatch && colonMatch[1]) {
        const p = parseInt(colonMatch[1], 10);
        if (!isNaN(p) && p > 0 && p <= 65535) {
            return p;
        }
    }
    
    // Try to match any standalone 4 or 5 digit number that could be a port
    const genericPortRegex = /\b(\d{4,5})\b/g;
    let match;
    while ((match = genericPortRegex.exec(cmd)) !== null) {
        const p = parseInt(match[1], 10);
        if (!isNaN(p) && p >= 1024 && p <= 65535) {
            return p;
        }
    }
    
    return null;
}

export function populateManagedProcessesDropdown(selectedValue) {
    const select = document.getElementById('proxy-managed-process-id');
    if (!select) return;
    select.innerHTML = '<option value="">None</option>';
    
    // Add change event listener if not already added
    if (!select.dataset.listenerAdded) {
        select.addEventListener('change', (e) => {
            const procId = e.target.value;
            if (procId) {
                const processes = managedState.allManagedProcesses || [];
                const proc = processes.find(p => p.id === procId);
                if (proc) {
                    const port = extractPortFromProcess(proc);
                    if (port) {
                        const targetInput = document.getElementById('proxy-target');
                        if (targetInput) {
                            targetInput.value = `http://127.0.0.1:${port}`;
                        }
                    }
                }
            }
        });
        select.dataset.listenerAdded = 'true';
    }
    
    // Read from live-bound import from managed.js
    const processes = managedState.allManagedProcesses || [];
    processes.forEach(proc => {
        const opt = document.createElement('option');
        opt.value = proc.id;
        opt.innerText = proc.name;
        if (proc.id === selectedValue) {
            opt.selected = true;
        }
        select.appendChild(opt);
    });
}

export function openAddProxyModal() {
    const idVal = document.getElementById('proxy-id-val');
    const nameInput = document.getElementById('proxy-name');
    const domInput = document.getElementById('proxy-domain');
    const alternativeDomInput = document.getElementById('proxy-alternative-domain');
    const pathInput = document.getElementById('proxy-path');
    const targetInput = document.getElementById('proxy-target');
    const spCheck = document.getElementById('proxy-strip-path');
    const enCheck = document.getElementById('proxy-enabled');
    const sslCheck = document.getElementById('proxy-ssl-enabled');

    if (idVal) idVal.value = '';
    if (nameInput) nameInput.value = '';
    if (domInput) domInput.value = '';
    if (alternativeDomInput) alternativeDomInput.value = '';
    if (pathInput) pathInput.value = '/';
    if (targetInput) targetInput.value = '';
    if (spCheck) spCheck.checked = false;
    if (enCheck) enCheck.checked = true;
    if (sslCheck) sslCheck.checked = false;
    
    populateManagedProcessesDropdown('');

    const title = document.getElementById('modal-proxy-title');
    const submitBtn = document.getElementById('btn-proxy-submit');
    if (title) title.innerText = 'Add Reverse Proxy Rule';
    if (submitBtn) submitBtn.innerText = 'Add Rule';

    const modal = document.getElementById('add-proxy-modal');
    if (modal) modal.classList.add('active');
}

export function openEditProxyModal(id) {
    const rule = allProxyRules.find(r => r.id === id);
    if (!rule) return;

    const idVal = document.getElementById('proxy-id-val');
    const nameInput = document.getElementById('proxy-name');
    const domInput = document.getElementById('proxy-domain');
    const alternativeDomInput = document.getElementById('proxy-alternative-domain');
    const pathInput = document.getElementById('proxy-path');
    const targetInput = document.getElementById('proxy-target');
    const spCheck = document.getElementById('proxy-strip-path');
    const enCheck = document.getElementById('proxy-enabled');
    const sslCheck = document.getElementById('proxy-ssl-enabled');

    if (idVal) idVal.value = rule.id;
    if (nameInput) nameInput.value = rule.name;
    if (domInput) domInput.value = rule.domain;
    if (alternativeDomInput) alternativeDomInput.value = rule.alternative_domain || '';
    if (pathInput) pathInput.value = rule.path;
    if (targetInput) targetInput.value = rule.target;
    if (spCheck) spCheck.checked = rule.strip_path;
    if (enCheck) enCheck.checked = rule.enabled;
    if (sslCheck) sslCheck.checked = rule.ssl_enabled;

    populateManagedProcessesDropdown(rule.managed_process_id || '');

    const title = document.getElementById('modal-proxy-title');
    const submitBtn = document.getElementById('btn-proxy-submit');
    if (title) title.innerText = 'Edit Reverse Proxy Rule';
    if (submitBtn) submitBtn.innerText = 'Save Changes';

    const modal = document.getElementById('add-proxy-modal');
    if (modal) modal.classList.add('active');
}

export function closeAddProxyModal() {
    const modal = document.getElementById('add-proxy-modal');
    if (modal) modal.classList.remove('active');
}

export function submitAddProxy() {
    const idVal = document.getElementById('proxy-id-val');
    const nameInput = document.getElementById('proxy-name');
    const domInput = document.getElementById('proxy-domain');
    const alternativeDomInput = document.getElementById('proxy-alternative-domain');
    const pathInput = document.getElementById('proxy-path');
    const targetInput = document.getElementById('proxy-target');
    const spCheck = document.getElementById('proxy-strip-path');
    const enCheck = document.getElementById('proxy-enabled');
    const sslCheck = document.getElementById('proxy-ssl-enabled');
    const mpSelect = document.getElementById('proxy-managed-process-id');

    const id = idVal ? idVal.value : '';
    const name = nameInput ? nameInput.value.trim() : '';
    const domain = domInput ? sanitizeHost(domInput.value) : '';
    const alternative_domain = alternativeDomInput ? sanitizeHost(alternativeDomInput.value) : '';
    const path = pathInput ? pathInput.value.trim() : '';
    const target = targetInput ? targetInput.value.trim() : '';
    const strip_path = spCheck ? spCheck.checked : false;
    const enabled = enCheck ? enCheck.checked : true;
    const ssl_enabled = sslCheck ? sslCheck.checked : false;
    const managed_process_id = mpSelect ? mpSelect.value : '';

    if (!name || !path || !target) {
        showToast('warning', 'Name, Path, and Target Destination are required');
        return;
    }

    const url = id ? '/api/proxy/update' : '/api/proxy/add';
    const body = { name, domain, alternative_domain, path, target, strip_path, enabled, ssl_enabled, managed_process_id };
    if (id) {
        body.id = id;
    }

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
            closeAddProxyModal();
            loadProxyRules();
        } else {
            showToast('error', res.message || 'Failed to save proxy rule');
        }
    })
    .catch(err => {
        showToast('error', 'Error connecting to server');
    });
}

export function toggleProxy(id, enabled) {
    fetch('/api/proxy/toggle', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ id, enabled })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            loadProxyRules();
        } else {
            showToast('error', res.message || 'Failed to toggle proxy rule status');
        }
    })
    .catch(err => {
        showToast('error', 'Error connecting to server');
    });
}

export function deleteProxy(id) {
    if (!confirm('Are you sure you want to delete this reverse proxy rule?')) {
        return;
    }

    fetch('/api/proxy/delete', {
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
            showToast('success', res.message);
            loadProxyRules();
        } else {
            showToast('error', res.message || 'Failed to delete proxy rule');
        }
    })
    .catch(err => {
        showToast('error', 'Error connecting to server');
    });
}
