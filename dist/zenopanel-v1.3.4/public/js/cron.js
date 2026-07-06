import { getCSRFToken } from './utils.js';
import { showToast } from './toast.js';

export let allCronJobs = [];

export async function loadCronJobs() {
    try {
        const response = await fetch('/api/cron/list');
        if (!response.ok) {
            throw new Error('Failed to fetch cron jobs');
        }
        const resData = await response.json();
        allCronJobs = resData.data || [];
        renderCronJobs();
    } catch (err) {
        showToast('error', 'Error loading cron jobs: ' + err.message);
    }
}

export function renderCronJobs() {
    const tbody = document.getElementById('cron-table-body');
    if (!tbody) return;

    if (allCronJobs.length === 0) {
        tbody.innerHTML = `<tr><td colspan="6" style="text-align:center; color:var(--text-muted); padding:20px;">No cron jobs found</td></tr>`;
        return;
    }

    tbody.innerHTML = allCronJobs.map(job => {
        const statusBadge = job.is_active === 1
            ? `<span style="display: inline-flex; align-items: center; gap: 6px; background: rgba(16, 185, 129, 0.1); color: #34d399; padding: 4px 10px; border-radius: 20px; font-size: 0.75rem; font-weight: 600; cursor: pointer;" onclick="toggleCronJob(${job.id}, 0)">
                   <span style="width: 6px; height: 6px; background: #34d399; border-radius: 50%;"></span> Active
               </span>`
            : `<span style="display: inline-flex; align-items: center; gap: 6px; background: rgba(239, 68, 68, 0.1); color: #f87171; padding: 4px 10px; border-radius: 20px; font-size: 0.75rem; font-weight: 600; cursor: pointer;" onclick="toggleCronJob(${job.id}, 1)">
                   <span style="width: 6px; height: 6px; background: #f87171; border-radius: 50%;"></span> Disabled
               </span>`;

        const lastRunTime = job.last_run ? job.last_run : 'Never';
        
        // Check if the command refers to an editable script file (absolute path or file extension)
        const cleanCommand = job.command.trim();
        const isScript = cleanCommand.startsWith('/') || cleanCommand.startsWith('./') || cleanCommand.includes('.sh') || cleanCommand.includes('.py') || cleanCommand.includes('.js');
        const editScriptBtn = isScript
            ? `<button class="btn-action" title="Edit Script File" onclick="window.editFile('${cleanCommand}')" style="background: rgba(168, 85, 247, 0.15); border: none; color: #c084fc; padding: 6px 10px; border-radius: 6px; cursor: pointer; margin-right: 4px; transition: all 0.2s;"><i class="fa-solid fa-file-signature"></i></button>`
            : '';

        return `
            <tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04); transition: background 0.15s ease;">
                <td style="padding: 14px 16px; font-weight: 600; color: #ffffff;">${job.name}</td>
                <td style="padding: 14px 16px; font-family: monospace; color: #a78bfa;">${job.schedule}</td>
                <td style="padding: 14px 16px; font-family: monospace; color: #cbd5e1; font-size: 0.8rem; background: rgba(0, 0, 0, 0.2); border-radius: 6px; padding: 4px 8px; max-width: 250px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: inline-block; margin-top: 10px;">${job.command}</td>
                <td style="padding: 14px 16px; color: #a1a1aa;">${lastRunTime}</td>
                <td style="padding: 14px 16px;">${statusBadge}</td>
                <td style="padding: 14px 16px; text-align:right;">
                    ${editScriptBtn}
                    <button class="btn-action" title="Run Now" onclick="runCronJob(${job.id}, '${job.name}')" style="background: rgba(99, 102, 241, 0.15); border: none; color: #818cf8; padding: 6px 10px; border-radius: 6px; cursor: pointer; margin-right: 4px; transition: all 0.2s;"><i class="fa-solid fa-play"></i></button>
                    <button class="btn-action" title="Delete" onclick="deleteCronJob(${job.id}, '${job.name}')" style="background: rgba(239, 68, 68, 0.15); border: none; color: #f87171; padding: 6px 10px; border-radius: 6px; cursor: pointer; transition: all 0.2s;"><i class="fa-solid fa-trash-can"></i></button>
                </td>
            </tr>
        `;
    }).join('');
}

export function openAddCronModal() {
    document.getElementById('cron-name').value = '';
    document.getElementById('cron-schedule').value = '';
    document.getElementById('cron-command').value = '';
    document.getElementById('add-cron-modal').classList.add('active');
}

export function closeAddCronModal() {
    document.getElementById('add-cron-modal').classList.remove('active');
}

export async function submitAddCron() {
    const name = document.getElementById('cron-name').value.trim();
    const schedule = document.getElementById('cron-schedule').value.trim();
    const command = document.getElementById('cron-command').value.trim();

    if (!name || !schedule || !command) {
        showToast('error', 'All fields are required');
        return;
    }

    const csrfToken = getCSRFToken();

    try {
        const response = await fetch('/api/cron/create', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({ name, schedule, command })
        });

        const data = await response.json();
        if (data.success) {
            showToast('success', 'Cron job created successfully');
            closeAddCronModal();
            loadCronJobs();
        } else {
            showToast('error', data.message || 'Failed to create cron job');
        }
    } catch (err) {
        showToast('error', 'Error: ' + err.message);
    }
}

export async function toggleCronJob(id, isActive) {
    const csrfToken = getCSRFToken();
    try {
        const response = await fetch('/api/cron/toggle', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({ id, is_active: isActive })
        });
        const data = await response.json();
        if (data.success) {
            showToast('success', data.message || 'Cron job updated');
            loadCronJobs();
        } else {
            showToast('error', data.message || 'Failed to update cron job');
        }
    } catch (err) {
        showToast('error', 'Error: ' + err.message);
    }
}

export async function deleteCronJob(id, name) {
    if (!confirm(`Are you sure you want to delete cron job "${name}"?`)) {
        return;
    }

    const csrfToken = getCSRFToken();
    try {
        const response = await fetch('/api/cron/delete', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({ id })
        });
        const data = await response.json();
        if (data.success) {
            showToast('success', 'Cron job deleted successfully');
            loadCronJobs();
        } else {
            showToast('error', data.message || 'Failed to delete cron job');
        }
    } catch (err) {
        showToast('error', 'Error: ' + err.message);
    }
}

export async function runCronJob(id, name) {
    showToast('info', `Executing cron job "${name}" immediately...`);
    const csrfToken = getCSRFToken();
    try {
        const response = await fetch('/api/cron/run', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({ id })
        });
        const data = await response.json();
        if (data.success) {
            if (data.exit_code === 0) {
                showToast('success', `Cron job "${name}" completed successfully! Output: ${data.stdout.trim() || '(None)'}`);
            } else {
                showToast('error', `Cron job "${name}" failed with code ${data.exit_code}. Error: ${data.stderr.trim()}`);
            }
            loadCronJobs();
        } else {
            showToast('error', data.message || 'Failed to run cron job');
        }
    } catch (err) {
        showToast('error', 'Error: ' + err.message);
    }
}

export function updateCronExplanation(val) {
    const el = document.getElementById('cron-explanation');
    if (!el) return;

    if (!val) {
        el.innerText = 'Silakan pilih preset atau ketik manual.';
        el.style.color = '#71717a';
        return;
    }

    const parts = val.trim().split(/\s+/);
    if (parts.length !== 5) {
        el.innerText = 'Format tidak valid. Harus memiliki 5 parameter (Menit Jam Hari Bulan Hari-Minggu).';
        el.style.color = '#f87171'; // Red
        return;
    }

    const [min, hour, day, month, dow] = parts;
    let desc = 'Dijalankan ';

    if (min === '*' && hour === '*' && day === '*' && month === '*' && dow === '*') {
        desc += 'setiap menit.';
    } else if (min.startsWith('*/') && hour === '*' && day === '*' && month === '*' && dow === '*') {
        desc += `setiap ${min.split('/')[1]} menit.`;
    } else if (min === '0' && hour === '*' && day === '*' && month === '*' && dow === '*') {
        desc += 'setiap jam pada menit ke-0.';
    } else if (min === '0' && hour === '0' && day === '*' && month === '*' && dow === '*') {
        desc += 'setiap hari pada pukul 00:00 (Tengah malam).';
    } else if (min === '0' && hour !== '*' && day === '*' && month === '*' && dow === '*') {
        desc += `setiap hari pada pukul ${hour.padStart(2, '0')}:00.`;
    } else if (min.match(/^\d+$/) && hour.match(/^\d+$/) && day === '*' && month === '*' && dow === '*') {
        desc += `setiap hari pada pukul ${hour.padStart(2, '0')}:${min.padStart(2, '0')}.`;
    } else if (min === '0' && hour === '0' && day === '*' && month === '*' && dow !== '*') {
        const days = ['Minggu', 'Senin', 'Selasa', 'Rabu', 'Kamis', 'Jumat', 'Sabtu'];
        const dayNames = dow.split(',').map(d => days[parseInt(d)] || d).join(', ');
        desc += `setiap hari ${dayNames} pada pukul 00:00.`;
    } else if (min === '0' && hour === '0' && day !== '*' && month === '*' && dow === '*') {
        desc += `setiap tanggal ${day} pukul 00:00.`;
    } else {
        desc += `berdasarkan jadwal kustom: [Menit: ${min}, Jam: ${hour}, Hari: ${day}, Bulan: ${month}, Hari-Minggu: ${dow}].`;
    }

    el.innerText = desc;
    el.style.color = '#34d399'; // Green
}

export function applyCronPreset(val) {
    if (val === 'custom') return;
    const scheduleInput = document.getElementById('cron-schedule');
    if (scheduleInput) {
        scheduleInput.value = val;
        updateCronExplanation(val);
    }
}

export function applyCronCommandPreset(val) {
    const cmdInput = document.getElementById('cron-command');
    if (!cmdInput) return;

    if (val === 'custom') {
        cmdInput.value = '';
    } else {
        cmdInput.value = val;
    }
}
