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
    } catch (err) {
        showToast('Error loading settings: ' + err.message, 'error');
    }
}

export async function submitSaveSettings() {
    const entrancePathInput = document.getElementById('settings-entrance-path');
    if (!entrancePathInput) return;
    
    const entrancePath = entrancePathInput.value.trim();
    if (!entrancePath) {
        showToast('Entrance path tidak boleh kosong', 'error');
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
            showToast(data.message || 'Pengaturan berhasil disimpan', 'success');
            // Refresh settings view
            entrancePathInput.value = data.entrance_path;
        } else {
            showToast(data.message || 'Gagal menyimpan pengaturan', 'error');
        }
    } catch (err) {
        showToast('Error saving settings: ' + err.message, 'error');
    } finally {
        btn.disabled = false;
        btn.innerHTML = originalText;
    }
}
