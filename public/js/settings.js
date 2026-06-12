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

