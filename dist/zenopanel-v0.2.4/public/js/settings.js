import { getCSRFToken } from './utils.js';
import { showToast } from './toast.js';

export async function loadSettings() {
    try {
        const response = await fetch('/api/settings');
        if (!response.ok) {
            if (response.status === 403) return; // Silent if not admin
            throw new Error('Failed to fetch settings');
        }
        const data = await response.json();
        if (data.success) {
            const pathInput = document.getElementById('settings-entrance-path');
            if (pathInput) {
                pathInput.value = data.entrance_path;
            }
        }
        await loadServiceStatus();
        await loadBackupSettings();
        await loadLogSettings();
    } catch (err) {
        showToast('error', 'Error loading settings: ' + err.message);
    }
}

export async function submitSaveSettings() {
    const entrancePathInput = document.getElementById('settings-entrance-path');
    if (!entrancePathInput) return;
    
    const entrancePath = entrancePathInput.value.trim();
    if (!entrancePath) {
        showToast('error', 'Entrance path tidak boleh kosong');
        return;
    }

    const csrfToken = getCSRFToken();
    const btn = document.getElementById('btn-save-settings');
    const originalText = btn.innerHTML;
    
    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Saving...';
        
        const response = await fetch('/api/settings', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({ entrance_path: entrancePath })
        });

        const data = await response.json();
        if (data.success) {
            showToast('success', data.message || 'Pengaturan berhasil disimpan');
            // Refresh settings view
            entrancePathInput.value = data.entrance_path;
        } else {
            showToast('error', data.message || 'Gagal menyimpan pengaturan');
        }
    } catch (err) {
        showToast('error', 'Error saving settings: ' + err.message);
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

export async function loadServiceStatus() {
    try {
        const response = await fetch('/api/settings/service-status');
        if (!response.ok) {
            throw new Error('Failed to fetch service status');
        }
        const res = await response.json();
        if (res.success && res.data) {
            const data = res.data;
            document.getElementById('service-distro').textContent = data.distro;
            document.getElementById('service-init-sys').textContent = data.init_system;
            
            const isRootEl = document.getElementById('service-is-root');
            if (data.is_root) {
                isRootEl.innerHTML = '<span style="color:#22c55e;"><i class="fa-solid fa-circle-check"></i> Yes</span>';
            } else {
                isRootEl.innerHTML = '<span style="color:#ef4444;"><i class="fa-solid fa-triangle-exclamation"></i> No (Running as user)</span>';
            }

            const badge = document.getElementById('service-status-badge');
            const installBtn = document.getElementById('btn-install-service');
            const uninstallBtn = document.getElementById('btn-uninstall-service');
            const manualContainer = document.getElementById('manual-instructions-container');

            if (data.status === 'active') {
                badge.textContent = 'Active (Running)';
                badge.style.background = 'rgba(34, 197, 94, 0.15)';
                badge.style.color = '#22c55e';
                badge.style.border = '1px solid rgba(34, 197, 94, 0.3)';
                
                installBtn.style.display = 'none';
                uninstallBtn.style.display = 'inline-block';
                
                if (!data.is_root) {
                    manualContainer.style.display = 'block';
                    document.getElementById('manual-title-label').textContent = 'Manual Service Uninstall';
                    document.getElementById('manual-config-sublabel').style.display = 'none';
                    document.getElementById('manual-config-box').style.display = 'none';
                    document.getElementById('manual-cmd-sublabel').textContent = 'Jalankan perintah ini di terminal Anda untuk mencopot service secara manual:';
                    document.getElementById('manual-install-cmd').value = data.uninstall_command;
                } else {
                    manualContainer.style.display = 'none';
                }
            } else if (data.status === 'inactive') {
                badge.textContent = 'Inactive (Stopped)';
                badge.style.background = 'rgba(234, 179, 8, 0.15)';
                badge.style.color = '#eab308';
                badge.style.border = '1px solid rgba(234, 179, 8, 0.3)';
                
                installBtn.style.display = 'inline-block';
                installBtn.innerHTML = '<i class="fa-solid fa-play"></i> Start Service';
                uninstallBtn.style.display = 'inline-block';
                
                if (!data.is_root) {
                    manualContainer.style.display = 'block';
                    document.getElementById('manual-title-label').textContent = 'Manual Service Uninstall';
                    document.getElementById('manual-config-sublabel').style.display = 'none';
                    document.getElementById('manual-config-box').style.display = 'none';
                    document.getElementById('manual-cmd-sublabel').textContent = 'Jalankan perintah ini di terminal Anda untuk mencopot service secara manual:';
                    document.getElementById('manual-install-cmd').value = data.uninstall_command;
                } else {
                    manualContainer.style.display = 'none';
                }
            } else {
                badge.textContent = 'Not Installed';
                badge.style.background = 'rgba(156, 163, 175, 0.15)';
                badge.style.color = '#9ca3af';
                badge.style.border = '1px solid rgba(156, 163, 175, 0.3)';
                
                installBtn.style.display = 'inline-block';
                installBtn.innerHTML = '<i class="fa-solid fa-plus"></i> Install Service';
                uninstallBtn.style.display = 'none';
                
                // Show manual instructions
                manualContainer.style.display = 'block';
                document.getElementById('manual-title-label').textContent = 'Manual Service Configuration';
                document.getElementById('manual-config-sublabel').style.display = 'block';
                document.getElementById('manual-config-box').style.display = 'block';
                document.getElementById('manual-cmd-sublabel').textContent = 'Jalankan perintah ini di terminal Anda untuk mendaftarkan service secara manual:';
                document.getElementById('service-config-content').textContent = data.service_content;
                document.getElementById('manual-install-cmd').value = data.install_command;
            }
        }
    } catch (err) {
        showToast('error', 'Error loading service status: ' + err.message);
    }
}

export async function installService() {
    const csrfToken = getCSRFToken();
    const btn = document.getElementById('btn-install-service');
    const originalText = btn.innerHTML;
    
    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Installing...';
        
        const response = await fetch('/api/settings/service-install', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            }
        });
        
        const data = await response.json();
        if (data.success) {
            showToast('success', data.message || 'Service berhasil diinstall');
            await loadServiceStatus();
        } else {
            showToast('error', data.message || 'Gagal menginstall service secara otomatis');
            await loadServiceStatus();
        }
    } catch (err) {
        showToast('error', 'Error installing service: ' + err.message);
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

export async function uninstallService() {
    if (!confirm('Apakah Anda yakin ingin mencopot service Zenopanel?')) return;
    
    const csrfToken = getCSRFToken();
    const btn = document.getElementById('btn-uninstall-service');
    const originalText = btn.innerHTML;
    
    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Uninstalling...';
        
        const response = await fetch('/api/settings/service-uninstall', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            }
        });
        
        const data = await response.json();
        if (data.success) {
            showToast('success', data.message || 'Service berhasil dicopot');
            await loadServiceStatus();
        } else {
            showToast('error', data.message || 'Gagal mencopot service secara otomatis');
            await loadServiceStatus();
        }
    } catch (err) {
        showToast('error', 'Error uninstalling service: ' + err.message);
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

export function copyInstallCmd() {
    const input = document.getElementById('manual-install-cmd');
    if (!input) return;
    input.select();
    navigator.clipboard.writeText(input.value)
        .then(() => showToast('success', 'Perintah berhasil disalin!'))
        .catch(() => showToast('error', 'Gagal menyalin perintah'));
}

export function toggleRateLimitFields() {
    const rlCheckbox = document.getElementById('settings-rl-enabled');
    const rlEnabled = rlCheckbox ? rlCheckbox.checked : false;
    const details = document.getElementById('rate-limit-details');
    if (details) {
        details.style.display = rlEnabled ? 'flex' : 'none';
    }
}

export async function loadSecuritySettings() {
    try {
        const tbody = document.getElementById('waf-logs-tbody');
        if (tbody) {
            tbody.innerHTML = '<tr><td colspan="5" style="text-align:center; color:var(--text-muted); padding:30px;"><i class="fa-solid fa-spinner fa-spin"></i> Loading audit logs...</td></tr>';
        }

        const response = await fetch('/api/settings/security');
        if (!response.ok) {
            throw new Error('Failed to fetch security settings');
        }
        const data = await response.json();
        if (data.success) {
            const wafCheckbox = document.getElementById('settings-waf-enabled');
            if (wafCheckbox) {
                wafCheckbox.checked = data.settings.waf_enabled;
            }
            const rlCheckbox = document.getElementById('settings-rl-enabled');
            if (rlCheckbox) {
                rlCheckbox.checked = data.settings.rate_limit_enabled;
            }
            
            const maxInput = document.getElementById('settings-rl-max');
            if (maxInput) {
                maxInput.value = data.settings.rate_limit_max;
            }
            
            const windowInput = document.getElementById('settings-rl-window');
            if (windowInput) {
                windowInput.value = data.settings.rate_limit_window;
            }
            
            toggleRateLimitFields();

            if (tbody) {
                if (!data.logs || data.logs.length === 0) {
                    tbody.innerHTML = '<tr><td colspan="5" style="text-align:center; color:var(--text-muted); padding:30px;">Belum ada log aktivitas keamanan terdeteksi.</td></tr>';
                } else {
                    tbody.innerHTML = data.logs.map(log => `
                        <tr>
                            <td>${log.id}</td>
                            <td><span style="font-family:var(--font-code); color:var(--accent-primary);">${log.ip}</span></td>
                            <td><span class="badge" style="background:rgba(239, 44, 44, 0.12); color:var(--danger); border:1px solid rgba(239, 44, 44, 0.1);">${escapeHtml(log.reason)}</span></td>
                            <td><span style="font-family:var(--font-code);">${escapeHtml(log.target)}</span></td>
                            <td style="color:var(--text-muted);">${formatDate(log.timestamp)}</td>
                        </tr>
                    `).join('');
                }
            }
        }
    } catch (err) {
        showToast('error', 'Error loading security settings: ' + err.message);
    }
}

export async function submitSaveSecurity() {
    const wafEnabled = document.getElementById('settings-waf-enabled').checked;
    const rateLimitEnabled = document.getElementById('settings-rl-enabled').checked;
    const rateLimitMax = parseInt(document.getElementById('settings-rl-max').value.trim(), 10) || 100;
    const rateLimitWindow = parseInt(document.getElementById('settings-rl-window').value.trim(), 10) || 60;

    const csrfToken = getCSRFToken();
    const btn = document.getElementById('btn-save-security');
    const originalText = btn.innerHTML;
    
    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Saving...';
        
        const response = await fetch('/api/settings/security', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({
                waf_enabled: wafEnabled,
                rate_limit_enabled: rateLimitEnabled,
                rate_limit_max: rateLimitMax,
                rate_limit_window: rateLimitWindow
            })
        });

        const data = await response.json();
        if (data.success) {
            showToast('success', data.message || 'Pengaturan keamanan berhasil disimpan');
            await loadSecuritySettings();
        } else {
            showToast('error', data.message || 'Gagal menyimpan pengaturan keamanan');
        }
    } catch (err) {
        showToast('error', 'Error saving security settings: ' + err.message);
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

function escapeHtml(text) {
    if (!text) return '';
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

function formatDate(dateStr) {
    if (!dateStr) return '-';
    try {
        const date = new Date(dateStr);
        return date.toLocaleString();
    } catch (e) {
        return dateStr;
    }
}

export function toggleBackupFields() {
    const backupCheckbox = document.getElementById('settings-backup-enabled');
    const enabled = backupCheckbox ? backupCheckbox.checked : false;
    const fields = document.getElementById('backup-settings-fields');
    if (fields) {
        fields.style.display = enabled ? 'flex' : 'none';
    }
}

export async function loadBackupSettings() {
    try {
        const response = await fetch('/api/settings/backup');
        if (!response.ok) {
            throw new Error('Failed to fetch backup settings');
        }
        const data = await response.json();
        if (data.success && data.settings) {
            const settings = data.settings;
            
            const backupEnabled = document.getElementById('settings-backup-enabled');
            if (backupEnabled) backupEnabled.checked = settings.enabled;
            
            const backupInterval = document.getElementById('settings-backup-interval');
            if (backupInterval) backupInterval.value = settings.interval_hours;
            
            const backupRetention = document.getElementById('settings-backup-retention');
            if (backupRetention) backupRetention.value = settings.retention;
            
            const backupDestDir = document.getElementById('settings-backup-dest-dir');
            if (backupDestDir) backupDestDir.value = settings.dest_dir;
            
            const backupPostScript = document.getElementById('settings-backup-post-script');
            if (backupPostScript) backupPostScript.value = settings.post_script;
            
            const backupLastRun = document.getElementById('backup-last-run-val');
            if (backupLastRun) backupLastRun.textContent = settings.last_run ? formatDate(settings.last_run) : 'Never';
            
            const backupLastStatus = document.getElementById('backup-last-status-val');
            if (backupLastStatus) backupLastStatus.textContent = settings.last_status || 'No status available';
            
            toggleBackupFields();
        }
    } catch (err) {
        showToast('error', 'Error loading backup settings: ' + err.message);
    }
}

export async function submitSaveBackupSettings() {
    const backupEnabled = document.getElementById('settings-backup-enabled').checked;
    const backupInterval = parseInt(document.getElementById('settings-backup-interval').value.trim(), 10) || 24;
    const backupRetention = parseInt(document.getElementById('settings-backup-retention').value.trim(), 10) || 7;
    const backupDestDir = document.getElementById('settings-backup-dest-dir').value.trim() || '/var/lib/zenopanel/backups';
    const backupPostScript = document.getElementById('settings-backup-post-script').value.trim() || '';

    const csrfToken = getCSRFToken();
    const btn = document.getElementById('btn-save-backup-settings');
    const originalText = btn.innerHTML;
    
    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Saving...';
        
        const response = await fetch('/api/settings/backup', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({
                enabled: backupEnabled,
                interval_hours: backupInterval,
                retention: backupRetention,
                dest_dir: backupDestDir,
                post_script: backupPostScript
            })
        });

        const data = await response.json();
        if (data.success) {
            showToast('success', data.message || 'Pengaturan backup berhasil disimpan');
            await loadBackupSettings();
        } else {
            showToast('error', data.message || 'Gagal menyimpan pengaturan backup');
        }
    } catch (err) {
        showToast('error', 'Error saving backup settings: ' + err.message);
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

export async function triggerBackupManual() {
    if (!confirm('Apakah Anda yakin ingin memicu backup manual sekarang? Proses ini dapat memakan waktu beberapa saat.')) return;
    
    const csrfToken = getCSRFToken();
    const btn = document.getElementById('btn-trigger-backup');
    const originalText = btn.innerHTML;
    
    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Backing up...';
        
        const response = await fetch('/api/settings/backup/trigger', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            }
        });

        const data = await response.json();
        if (data.success) {
            showToast('success', `${data.message}. File: ${data.filename}`);
            await loadBackupSettings();
        } else {
            showToast('error', data.message + (data.error ? ': ' + data.error : ''));
            await loadBackupSettings();
        }
    } catch (err) {
        showToast('error', 'Error triggering backup: ' + err.message);
        await loadBackupSettings();
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

// ─────────────────────────────────────────────────────────
// LOG ROTATION SETTINGS
// ─────────────────────────────────────────────────────────

export async function loadLogSettings() {
    try {
        const response = await fetch('/api/settings/logs');
        const data = await response.json();
        if (data.success && data.data) {
            const d = data.data;

            const intervalEl = document.getElementById('log-rotation-interval');
            const maxSizeEl = document.getElementById('log-max-size-mb');
            const wafRetEl = document.getElementById('log-waf-retention-days');
            const lastRotEl = document.getElementById('log-last-rotation-val');
            const lastStatusEl = document.getElementById('log-last-status-val');

            if (intervalEl) intervalEl.value = d.interval_hours ?? 24;
            if (maxSizeEl) maxSizeEl.value = d.max_size_mb ?? 10;
            if (wafRetEl) wafRetEl.value = d.waf_retention_days ?? 30;
            if (lastRotEl) lastRotEl.textContent = d.last_rotation || '-';
            if (lastStatusEl) lastStatusEl.textContent = d.last_status || '-';
        }
    } catch (err) {
        console.error('Error loading log settings:', err);
    }
}

export async function submitSaveLogSettings() {
    const btn = document.getElementById('btn-save-log-settings');
    if (!btn) return;
    const originalText = btn.innerHTML;
    const csrfToken = getCSRFToken ? getCSRFToken() : '';

    const payload = {
        interval_hours: parseInt(document.getElementById('log-rotation-interval')?.value || '24'),
        max_size_mb: parseInt(document.getElementById('log-max-size-mb')?.value || '10'),
        waf_retention_days: parseInt(document.getElementById('log-waf-retention-days')?.value || '30'),
    };

    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Saving...';

        const response = await fetch('/api/settings/logs', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify(payload)
        });

        const data = await response.json();
        if (data.success) {
            showToast('success', data.message || 'Pengaturan log rotation berhasil disimpan');
        } else {
            showToast('error', data.message || 'Gagal menyimpan pengaturan');
        }
    } catch (err) {
        showToast('error', 'Error saving log settings: ' + err.message);
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

export async function triggerLogRotation() {
    const btn = document.getElementById('btn-trigger-log-rotation');
    if (!btn) return;
    const originalText = btn.innerHTML;
    const csrfToken = getCSRFToken ? getCSRFToken() : '';

    try {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Rotating...';

        const response = await fetch('/api/settings/logs/rotate', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            }
        });

        const data = await response.json();
        if (data.success) {
            showToast('success', `${data.message}. ${data.summary || ''}`);
            await loadLogSettings();
        } else {
            showToast('error', data.message || 'Gagal menjalankan log rotation');
            await loadLogSettings();
        }
    } catch (err) {
        showToast('error', 'Error triggering log rotation: ' + err.message);
        await loadLogSettings();
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}

