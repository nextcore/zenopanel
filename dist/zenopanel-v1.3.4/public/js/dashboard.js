import { getCSRFToken, escapeHtml } from './utils.js';
import { showToast } from './toast.js';

// Elements — cached once, reused on every SSE tick
let cpuRing = null;
let ramRing = null;
let diskRing = null;
let swapRing = null;
let elCpuVal = null;
let elRamVal = null;
let elRamSub = null;
let elDiskVal = null;
let elDiskSub = null;
let elSwapVal = null;
let elSwapSub = null;
let elNetRxVal = null;
let elNetTxVal = null;

// Stats history trackers
export let sysStatsInterval = null;
export let statsEventSource = null;
export let performanceChart = null;
export let trafficChart = null;
let chartDataPoints = { labels: [], cpu: [], ram: [] };
let trafficDataPoints = { labels: [], rps: [], latency: [] };
let lastRxBytes = 0;
let lastTxBytes = 0;
let lastNetCheckTime = 0;

function initRingElements() {
    if (!cpuRing) cpuRing = document.getElementById('cpu-ring');
    if (!ramRing) ramRing = document.getElementById('ram-ring');
    if (!diskRing) diskRing = document.getElementById('disk-ring');
    if (!swapRing) swapRing = document.getElementById('swap-ring');
    if (!elCpuVal) elCpuVal = document.getElementById('cpu-val');
    if (!elRamVal) elRamVal = document.getElementById('ram-val');
    if (!elRamSub) elRamSub = document.getElementById('ram-sub');
    if (!elDiskVal) elDiskVal = document.getElementById('disk-val');
    if (!elDiskSub) elDiskSub = document.getElementById('disk-sub');
    if (!elSwapVal) elSwapVal = document.getElementById('swap-val');
    if (!elSwapSub) elSwapSub = document.getElementById('swap-sub');
    if (!elNetRxVal) elNetRxVal = document.getElementById('net-rx-val');
    if (!elNetTxVal) elNetTxVal = document.getElementById('net-tx-val');
}

export function setRingProgress(ring, pct) {
    if (!ring) return;
    const radius = ring.r.baseVal.value;
    const circumference = 2 * Math.PI * radius;
    const offset = circumference - (pct / 100) * circumference;
    ring.style.strokeDashoffset = offset;
}

export function initPerformanceChart() {
    const chartEl = document.getElementById('performanceChart');
    if (!chartEl) return;
    
    if (performanceChart) {
        performanceChart.destroy();
        performanceChart = null;
    }
    const ctx = chartEl.getContext('2d');
    
    // Build gradient color
    const cpuGrad = ctx.createLinearGradient(0, 0, 0, 300);
    cpuGrad.addColorStop(0, 'rgba(59, 130, 246, 0.4)');
    cpuGrad.addColorStop(1, 'rgba(59, 130, 246, 0.0)');

    const ramGrad = ctx.createLinearGradient(0, 0, 0, 300);
    ramGrad.addColorStop(0, 'rgba(168, 85, 247, 0.4)');
    ramGrad.addColorStop(1, 'rgba(168, 85, 247, 0.0)');

    // Ensure Chart is available globally (loaded via CDN/script tag in head)
    if (typeof Chart === 'undefined') {
        console.error('Chart.js is not loaded');
        return;
    }

    performanceChart = new Chart(ctx, {
        type: 'line',
        data: {
            labels: chartDataPoints.labels,
            datasets: [
                {
                    label: 'CPU Usage %',
                    data: chartDataPoints.cpu,
                    borderColor: '#3b82f6',
                    borderWidth: 2,
                    backgroundColor: cpuGrad,
                    fill: true,
                    tension: 0.4,
                    pointRadius: 0,
                    pointHoverRadius: 4
                },
                {
                    label: 'RAM Usage %',
                    data: chartDataPoints.ram,
                    borderColor: '#a855f7',
                    borderWidth: 2,
                    backgroundColor: ramGrad,
                    fill: true,
                    tension: 0.4,
                    pointRadius: 0,
                    pointHoverRadius: 4
                }
            ]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            scales: {
                x: {
                    grid: { display: false },
                    ticks: { color: '#64748b', font: { size: 10 } }
                },
                y: {
                    min: 0,
                    max: 100,
                    grid: { color: 'rgba(255,255,255,0.03)' },
                    ticks: { color: '#64748b', font: { size: 10 } }
                }
            },
            plugins: {
                legend: {
                    labels: { color: '#f1f5f9', font: { size: 11 } }
                }
            }
        }
    });
}

export function initTrafficChart() {
    const chartEl = document.getElementById('trafficChart');
    if (!chartEl) return;
    
    if (trafficChart) {
        trafficChart.destroy();
        trafficChart = null;
    }
    const ctx = chartEl.getContext('2d');
    
    const rpsGrad = ctx.createLinearGradient(0, 0, 0, 300);
    rpsGrad.addColorStop(0, 'rgba(168, 85, 247, 0.4)');
    rpsGrad.addColorStop(1, 'rgba(168, 85, 247, 0.0)');

    const latGrad = ctx.createLinearGradient(0, 0, 0, 300);
    latGrad.addColorStop(0, 'rgba(16, 185, 129, 0.4)');
    latGrad.addColorStop(1, 'rgba(16, 185, 129, 0.0)');

    if (typeof Chart === 'undefined') {
        console.error('Chart.js is not loaded');
        return;
    }

    trafficChart = new Chart(ctx, {
        type: 'line',
        data: {
            labels: trafficDataPoints.labels,
            datasets: [
                {
                    label: 'RPS (Reqs/Sec)',
                    data: trafficDataPoints.rps,
                    borderColor: '#a855f7',
                    borderWidth: 2,
                    backgroundColor: rpsGrad,
                    fill: true,
                    tension: 0.4,
                    pointRadius: 0,
                    pointHoverRadius: 4,
                    yAxisID: 'y'
                },
                {
                    label: 'Latency (ms)',
                    data: trafficDataPoints.latency,
                    borderColor: '#10b981',
                    borderWidth: 2,
                    backgroundColor: latGrad,
                    fill: true,
                    tension: 0.4,
                    pointRadius: 0,
                    pointHoverRadius: 4,
                    yAxisID: 'y1'
                }
            ]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            scales: {
                x: {
                    grid: { display: false },
                    ticks: { color: '#64748b', font: { size: 10 } }
                },
                y: {
                    type: 'linear',
                    display: true,
                    position: 'left',
                    min: 0,
                    grid: { color: 'rgba(255,255,255,0.03)' },
                    ticks: { color: '#a855f7', font: { size: 10 } },
                    title: { display: true, text: 'RPS', color: '#a855f7', font: { size: 10, weight: 'bold' } }
                },
                y1: {
                    type: 'linear',
                    display: true,
                    position: 'right',
                    min: 0,
                    grid: { drawOnChartArea: false },
                    ticks: { color: '#10b981', font: { size: 10 } },
                    title: { display: true, text: 'Latency (ms)', color: '#10b981', font: { size: 10, weight: 'bold' } }
                }
            },
            plugins: {
                legend: {
                    labels: { color: '#f1f5f9', font: { size: 11 } }
                }
            }
        }
    });
}

export function updatePerformanceChart(cpuPct, ramPct) {
    if (!performanceChart) return;

    const now = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    chartDataPoints.labels.push(now);
    chartDataPoints.cpu.push(cpuPct);
    chartDataPoints.ram.push(ramPct);

    if (chartDataPoints.labels.length > 15) {
        chartDataPoints.labels.shift();
        chartDataPoints.cpu.shift();
        chartDataPoints.ram.shift();
    }

    performanceChart.update();
}

export function updateStatsUI(s) {
    // Use cached element refs — no getElementById on each tick
    if (elCpuVal) elCpuVal.innerText = s.cpu.toFixed(1) + '%';
    setRingProgress(cpuRing, s.cpu);

    if (elRamVal) elRamVal.innerText = s.mem_pct.toFixed(1) + '%';
    setRingProgress(ramRing, s.mem_pct);
    if (elRamSub) elRamSub.innerText = (s.mem_used / (1024*1024*1024)).toFixed(2) + ' / ' + (s.mem_total / (1024*1024*1024)).toFixed(2) + ' GB';

    if (elDiskVal) elDiskVal.innerText = s.disk_pct.toFixed(1) + '%';
    setRingProgress(diskRing, s.disk_pct);
    if (elDiskSub) elDiskSub.innerText = (s.disk_used / (1024*1024*1024)).toFixed(1) + ' / ' + (s.disk_total / (1024*1024*1024)).toFixed(1) + ' GB';

    // Swap
    if (s.swap_total !== undefined) {
        if (elSwapVal) elSwapVal.innerText = (s.swap_pct || 0).toFixed(1) + '%';
        if (elSwapSub) {
            const usedGB = (s.swap_used / (1024*1024*1024)).toFixed(2);
            const totalGB = (s.swap_total / (1024*1024*1024)).toFixed(2);
            elSwapSub.innerText = usedGB + ' / ' + totalGB + ' GB';
        }
        setRingProgress(swapRing, s.swap_pct || 0);
    }

    // Net speed
    const now = Date.now();
    if (lastNetCheckTime > 0) {
        const timeSec = (now - lastNetCheckTime) / 1000;
        const rxSpeedKB = ((s.net_rx - lastRxBytes) / 1024) / timeSec;
        const txSpeedKB = ((s.net_tx - lastTxBytes) / 1024) / timeSec;
        if (elNetRxVal) elNetRxVal.innerText = formatSpeed(rxSpeedKB);
        if (elNetTxVal) elNetTxVal.innerText = formatSpeed(txSpeedKB);
    }
    lastRxBytes = s.net_rx;
    lastTxBytes = s.net_tx;
    lastNetCheckTime = now;

    updatePerformanceChart(s.cpu, s.mem_pct);
}

export function loadSystemStats() {
    initRingElements();
    fetch('/api/stats')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                updateStatsUI(res.data);
            }
        })
        .catch(err => console.error("Error polling system stats:", err));
}

export function formatSpeed(speedKB) {
    if (speedKB > 1024) {
        return (speedKB / 1024).toFixed(1) + ' MB/s';
    }
    return speedKB.toFixed(1) + ' KB/s';
}

const STATIC_INFO_CACHE_KEY = 'zp_static_info';

export function loadStaticSystemInfo() {
    // Apply any cached data immediately (zero-latency render)
    const cached = sessionStorage.getItem(STATIC_INFO_CACHE_KEY);
    if (cached) {
        try {
            applyStaticInfo(JSON.parse(cached));
        } catch(e) {
            sessionStorage.removeItem(STATIC_INFO_CACHE_KEY);
        }
    }

    // Always fetch to get latest uptime + update check, and refresh cache
    fetch('/api/info')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const d = res.data;
                // Cache static fields (exclude uptime which changes every second)
                const toCache = { os: d.os, cores: d.cores, cpu_model: d.cpu_model, arch: d.arch, hostname: d.hostname, version: d.version };
                sessionStorage.setItem(STATIC_INFO_CACHE_KEY, JSON.stringify(toCache));
                applyStaticInfo(d);
            }
        })
        .catch(err => console.error(err));
}

function applyStaticInfo(d) {
    const sysOsEl = document.getElementById('sys-os');
    const sysUptimeEl = document.getElementById('sys-uptime');
    const cpuCoresEl = document.getElementById('cpu-cores');
    const cpuModelEl = document.getElementById('cpu-model-name');
    const cpuArchEl = document.getElementById('cpu-arch');
    const sysHostEl = document.getElementById('sys-host');

    if (sysOsEl && d.os) sysOsEl.innerText = d.os;
    if (sysUptimeEl && d.uptime) sysUptimeEl.innerText = 'Uptime: ' + d.uptime;
    if (cpuCoresEl && d.cores) cpuCoresEl.innerText = d.cores + ' Cores';
    if (cpuModelEl && d.cpu_model) cpuModelEl.innerText = d.cpu_model;
    if (cpuArchEl && d.arch) cpuArchEl.innerText = d.arch;
    if (sysHostEl && d.hostname) sysHostEl.innerText = d.hostname;

    // Update alert banner for new versions
    const bannerEl = document.getElementById('update-alert-banner');
    const latestVerStrEl = document.getElementById('latest-ver-str');
    const currentVerStrEl = document.getElementById('current-ver-str');
    if (bannerEl && d.update_available && d.latest_version) {
        if (latestVerStrEl) latestVerStrEl.innerText = 'v' + d.latest_version;
        if (currentVerStrEl) currentVerStrEl.innerText = 'v' + d.version;
        bannerEl.style.display = 'flex';
    } else if (bannerEl) {
        bannerEl.style.display = 'none';
    }
}

export function updateProcessesUI(procs) {
    const tbody = document.getElementById('process-table-body');
    if (!tbody) return;
    
    let html = '';
    procs.forEach(p => {
        // Determine badge state
        let stateClass = 'badge-sleeping';
        let stateLabel = 'Sleeping';
        if (p.status === 'R' || p.status === 'Running') { stateClass = 'badge-running'; stateLabel = 'Running'; }
        if (p.status === 'T' || p.status === 'Stopped') { stateClass = 'badge-stopped'; stateLabel = 'Stopped'; }

        const safeName = escapeHtml(p.name);

        html += `
            <tr>
                <td style="font-family:var(--font-code); font-weight:600;">${p.pid}</td>
                <td style="font-family:var(--font-code); max-width:240px; overflow:hidden; text-overflow:ellipsis;" title="${safeName}">${safeName}</td>
                <td>${p.cpu.toFixed(1)}%</td>
                <td>${p.memory.toFixed(1)}%</td>
                <td><span class="badge ${stateClass}">${stateLabel}</span></td>
                <td style="text-align:right;">
                    <button class="btn-icon" onclick="killProcess(${p.pid})" title="Kill process">
                        <i class="fa-solid fa-skull"></i>
                    </button>
                </td>
            </tr>
        `;
    });
    tbody.innerHTML = html;
}

export function loadProcesses() {
    fetch('/api/processes?limit=15')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                updateProcessesUI(res.data);
            }
        })
        .catch(err => console.error("Error loading processes:", err));
}

export function killProcess(pid) {
    if (confirm(`Apakah Anda yakin ingin mematikan paksa (SIGKILL) proses dengan PID: ${pid}?`)) {
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
                showToast('success', res.message || 'Proses berhasil dihentikan');
                loadProcesses();
            } else {
                showToast('error', res.message || 'Gagal mematikan proses');
            }
        })
        .catch(err => {
            showToast('error', 'Gagal memanggil API: ' + err.toString());
        });
    }
}

export function updateTrafficStatsUI(history) {
    trafficDataPoints.labels = [];
    trafficDataPoints.rps = [];
    trafficDataPoints.latency = [];
    
    const itemsToShow = history.slice(-15);
    
    itemsToShow.forEach(item => {
        const time = new Date(item.timestamp * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
        trafficDataPoints.labels.push(time);
        
        const rpsVal = item.requests / 2.0;
        trafficDataPoints.rps.push(rpsVal);
        
        const avgLat = item.requests > 0 ? (item.latency_ms / item.requests) : 0;
        trafficDataPoints.latency.push(avgLat);
    });

    if (history.length > 0) {
        const latest = history[history.length - 1];
        const currentRps = latest.requests / 2.0;
        const avgLatency = latest.requests > 0 ? (latest.latency_ms / latest.requests) : 0;
        
        const rxSpeedKB = (latest.bytes_received / 1024) / 2.0;
        const txSpeedKB = (latest.bytes_sent / 1024) / 2.0;

        const rpsEl = document.getElementById('traffic-rps');
        if (rpsEl) rpsEl.innerText = currentRps.toFixed(1) + ' reqs/s';
        
        const latEl = document.getElementById('traffic-latency');
        if (latEl) latEl.innerText = Math.round(avgLatency) + ' ms';

        const inEl = document.getElementById('traffic-in');
        if (inEl) inEl.innerText = formatSpeed(rxSpeedKB);

        const outEl = document.getElementById('traffic-out');
        if (outEl) outEl.innerText = formatSpeed(txSpeedKB);
    }

    if (trafficChart) {
        trafficChart.data.labels = trafficDataPoints.labels;
        trafficChart.data.datasets[0].data = trafficDataPoints.rps;
        trafficChart.data.datasets[1].data = trafficDataPoints.latency;
        trafficChart.update();
    }
}

export function loadTrafficStats() {
    fetch('/api/settings/analytics')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.history) {
                updateTrafficStatsUI(res.history);
            }
        })
        .catch(err => console.error("Error loading traffic stats:", err));
}

export function startStatsPolling() {
    loadContainerList();
    
    // Cache all dashboard DOM elements now (they exist since Dashboard tab just loaded)
    initRingElements();
    
    if (statsEventSource) {
        return;
    }
    
    statsEventSource = new EventSource('/api/stats/stream');
    
    statsEventSource.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            if (data.stats) {
                updateStatsUI(data.stats);
            }
            if (data.processes) {
                updateProcessesUI(data.processes);
            }
            if (data.history) {
                updateTrafficStatsUI(data.history);
            }
        } catch (e) {
            console.error("Gagal mengurai data SSE:", e);
        }
    };
    
    statsEventSource.onerror = (err) => {
        console.error("SSE stream error:", err);
    };
}

export function stopStatsPolling() {
    if (statsEventSource) {
        statsEventSource.close();
        statsEventSource = null;
    }
}

export function loadContainerList() {
    fetch('/api/containers/list')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const sel = document.getElementById('log-viewer-container-select');
                if (!sel) return;
                // Preserve current selection
                const current = sel.value;
                sel.innerHTML = '<option value="">— Pilih Kontainer —</option>';
                (res.data || []).forEach(c => {
                    const opt = document.createElement('option');
                    opt.value = c.name || c.id || '';
                    opt.textContent = (c.name || c.id || '') + (c.status ? ' [' + c.status + ']' : '');
                    sel.appendChild(opt);
                });
                if (current) sel.value = current;
            }
        })
        .catch(() => {}); // silently ignore if endpoint not ready
}

export function refreshContainerLog() {
    const sel = document.getElementById('log-viewer-container-select');
    const linesSel = document.getElementById('log-viewer-lines-select');
    const output = document.getElementById('log-viewer-output');
    if (!sel || !output) return;

    const containerId = sel.value;
    if (!containerId) {
        output.textContent = 'Pilih kontainer terlebih dahulu.';
        return;
    }

    const lines = linesSel ? linesSel.value : '200';
    output.textContent = 'Memuat log...';

    fetch(`/api/containers/${encodeURIComponent(containerId)}/logs?lines=${lines}`)
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const d = res.data;
                if (d.ok && d.lines && d.lines.length > 0) {
                    output.textContent = d.lines.join('\n');
                    // Auto scroll to bottom
                    output.scrollTop = output.scrollHeight;
                } else if (d.error) {
                    output.textContent = 'Error: ' + d.error;
                } else {
                    output.textContent = '(Log kosong atau tidak ada output)';
                }
            } else {
                output.textContent = 'Gagal memuat log: ' + (res.message || 'unknown error');
            }
        })
        .catch(err => {
            output.textContent = 'Error: ' + err.toString();
        });
}

export function triggerPanelUpdate() {
    if (!confirm('Are you sure you want to update ZenoPanel now? The panel server will restart during the update.')) {
        return;
    }

    const btn = document.getElementById('btn-update-panel');
    const actionsContainer = document.getElementById('update-banner-actions');
    if (btn) {
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Updating...';
    }

    fetch('/api/system/update', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        }
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', 'Update initiated! Reconnecting to ZenoPanel...');
            if (actionsContainer) {
                actionsContainer.innerHTML = '<span style="font-size:0.85rem; color:var(--text-muted);"><i class="fa-solid fa-sync fa-spin"></i> Server restarting, reconnecting...</span>';
            }
            setTimeout(checkServerReconnection, 3000);
        } else {
            showToast('error', res.message || 'Failed to start update');
            if (btn) {
                btn.disabled = false;
                btn.innerHTML = '<i class="fa-solid fa-cloud-arrow-down"></i> Update Now';
            }
        }
    })
    .catch(err => {
        showToast('error', 'Error sending update request');
        if (btn) {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-cloud-arrow-down"></i> Update Now';
        }
    });
}

function checkServerReconnection() {
    fetch('/api/info')
        .then(res => {
            if (res.ok) {
                showToast('success', 'ZenoPanel updated and online!');
                setTimeout(() => window.location.reload(), 1000);
            } else {
                setTimeout(checkServerReconnection, 2000);
            }
        })
        .catch(() => {
            setTimeout(checkServerReconnection, 2000);
        });
}
