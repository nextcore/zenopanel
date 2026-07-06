import { getCSRFToken, escapeHtml } from './utils.js';
import { showToast } from './toast.js';
import { loadManagedProcesses } from './managed.js';

export function openPortCheckModal() {
    const portNumInput = document.getElementById('check-port-num');
    if (portNumInput) portNumInput.value = '';
    const resPanel = document.getElementById('port-check-result');
    if (resPanel) {
        resPanel.style.display = 'none';
        resPanel.innerHTML = '';
    }
    
    const modal = document.getElementById('port-check-modal');
    if (modal) {
        modal.classList.add('active');
        if (portNumInput) portNumInput.focus();
    }
}

export function closePortCheckModal() {
    const modal = document.getElementById('port-check-modal');
    if (modal) modal.classList.remove('active');
}

export function submitPortCheck() {
    const portValInput = document.getElementById('check-port-num');
    const portVal = portValInput ? portValInput.value.trim() : '';
    if (!portVal) {
        showToast('warning', 'Please enter a port number');
        return;
    }

    const port = parseInt(portVal, 10);
    if (isNaN(port) || port < 1 || port > 65535) {
        showToast('warning', 'Port must be a number between 1 and 65535');
        return;
    }

    const resPanel = document.getElementById('port-check-result');
    if (resPanel) {
        resPanel.style.display = 'block';
        resPanel.innerHTML = '<div style="text-align:center; color:var(--text-muted);"><i class="fa-solid fa-spinner fa-spin"></i> Checking port...</div>';
    }

    fetch('/api/system/check_port', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ port: port })
    })
    .then(res => res.json())
    .then(res => {
        if (!resPanel) return;
        if (res.success && res.data) {
            const info = res.data;
            if (info.in_use) {
                resPanel.innerHTML = 
                    '<div style="display:flex; flex-direction:column; gap:12px;">' +
                        '<div style="display:flex; align-items:center; gap:8px; color:var(--danger); font-weight:600;">' +
                            '<i class="fa-solid fa-triangle-exclamation"></i> Port ' + port + ' is ALREADY IN USE!' +
                        '</div>' +
                        '<div style="font-size:0.9rem; line-height:1.5;">' +
                            '<div style="margin-bottom:4px;"><strong>Process Name:</strong> <code style="background:rgba(255,255,255,0.06); padding:2px 6px; border-radius:4px; font-family:var(--font-code); color:var(--text-main);">' + escapeHtml(info.process_name || 'unknown') + '</code></div>' +
                            '<div><strong>PID:</strong> <code style="background:rgba(255,255,255,0.06); padding:2px 6px; border-radius:4px; font-family:var(--font-code); color:var(--text-main);">' + info.pid + '</code></div>' +
                        '</div>' +
                        '<div style="display:flex; justify-content:flex-end; margin-top:8px;">' +
                            '<button class="btn-danger-action" onclick="killPortProcess(' + info.pid + ', ' + port + ')" style="display:flex; align-items:center; gap:6px; background:var(--danger); color:#fff; border:none; padding:8px 14px; border-radius:6px; font-weight:600; cursor:pointer;">' +
                                '<i class="fa-solid fa-skull"></i> Kill Process (PID ' + info.pid + ')' +
                            '</button>' +
                        '</div>' +
                    '</div>';
            } else {
                resPanel.innerHTML = 
                    '<div style="display:flex; align-items:center; gap:8px; color:var(--success); font-weight:600;">' +
                        '<i class="fa-solid fa-circle-check"></i> Port ' + port + ' is FREE (Not in use)' +
                    '</div>';
            }
        } else {
            resPanel.innerHTML = `<div style="color:var(--danger)">Error: ${res.message || 'Failed to check port status'}</div>`;
        }
    })
    .catch(err => {
        if (resPanel) {
            resPanel.innerHTML = `<div style="color:var(--danger)">Error checking port: ${err.toString()}</div>`;
        }
    });
}

export function killPortProcess(pid, port) {
    if (!confirm(`Are you sure you want to KILL process PID ${pid} running on port ${port}?`)) {
        return;
    }

    showToast('warning', `Killing process PID ${pid}...`);

    fetch('/api/processes/kill', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ pid: pid })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', `Process PID ${pid} successfully killed`);
            submitPortCheck(); // Refresh port check status
            loadManagedProcesses(); // Refresh managed process list
        } else {
            showToast('error', res.message || 'Failed to kill process');
        }
    })
    .catch(err => {
        showToast('error', 'Error sending kill request: ' + err.toString());
    });
}
