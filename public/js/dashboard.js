import { getCSRFToken, escapeHtml } from './utils.js';
import { showToast } from './toast.js';

// Elements
let cpuRing = null;
let ramRing = null;
let diskRing = null;

// Stats history trackers
export let sysStatsInterval = null;
export let performanceChart = null;
let chartDataPoints = { labels: [], cpu: [], ram: [] };
let lastRxBytes = 0;
let lastTxBytes = 0;
let lastNetCheckTime = 0;

function initRingElements() {
    if (!cpuRing) cpuRing = document.getElementById('cpu-ring');
    if (!ramRing) ramRing = document.getElementById('ram-ring');
    if (!diskRing) diskRing = document.getElementById('disk-ring');
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

export function loadSystemStats() {
    initRingElements();
    fetch('/api/stats')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const s = res.data;
                
                // Update values
                const cpuValEl = document.getElementById('cpu-val');
                if (cpuValEl) cpuValEl.innerText = s.cpu.toFixed(1) + '%';
                setRingProgress(cpuRing, s.cpu);

                const ramValEl = document.getElementById('ram-val');
                if (ramValEl) ramValEl.innerText = s.mem_pct.toFixed(1) + '%';
                setRingProgress(ramRing, s.mem_pct);
                
                const ramSubEl = document.getElementById('ram-sub');
                if (ramSubEl) ramSubEl.innerText = (s.mem_used / (1024*1024*1024)).toFixed(2) + ' / ' + (s.mem_total / (1024*1024*1024)).toFixed(2) + ' GB';

                const diskValEl = document.getElementById('disk-val');
                if (diskValEl) diskValEl.innerText = s.disk_pct.toFixed(1) + '%';
                setRingProgress(diskRing, s.disk_pct);
                
                const diskSubEl = document.getElementById('disk-sub');
                if (diskSubEl) diskSubEl.innerText = (s.disk_used / (1024*1024*1024)).toFixed(1) + ' / ' + (s.disk_total / (1024*1024*1024)).toFixed(1) + ' GB';

                // Net speed calculations
                const now = Date.now();
                if (lastNetCheckTime > 0) {
                    const timeSec = (now - lastNetCheckTime) / 1000;
                    const rxDiff = s.net_rx - lastRxBytes;
                    const txDiff = s.net_tx - lastTxBytes;

                    const rxSpeedKB = (rxDiff / 1024) / timeSec;
                    const txSpeedKB = (txDiff / 1024) / timeSec;

                    const rxValEl = document.getElementById('net-rx-val');
                    const txValEl = document.getElementById('net-tx-val');
                    if (rxValEl) rxValEl.innerText = formatSpeed(rxSpeedKB);
                    if (txValEl) txValEl.innerText = formatSpeed(txSpeedKB);
                }

                lastRxBytes = s.net_rx;
                lastTxBytes = s.net_tx;
                lastNetCheckTime = now;

                // Update chart
                updatePerformanceChart(s.cpu, s.mem_pct);
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

export function loadStaticSystemInfo() {
    fetch('/api/info')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const d = res.data;
                const sysOsEl = document.getElementById('sys-os');
                const sysUptimeEl = document.getElementById('sys-uptime');
                const cpuCoresEl = document.getElementById('cpu-cores');
                const cpuModelEl = document.getElementById('cpu-model-name');
                const cpuArchEl = document.getElementById('cpu-arch');
                const sysHostEl = document.getElementById('sys-host');

                if (sysOsEl) sysOsEl.innerText = d.os;
                if (sysUptimeEl) sysUptimeEl.innerText = 'Uptime: ' + d.uptime;
                if (cpuCoresEl) cpuCoresEl.innerText = d.cores + ' Cores';
                if (cpuModelEl) cpuModelEl.innerText = d.cpu_model;
                if (cpuArchEl) cpuArchEl.innerText = d.arch;
                if (sysHostEl) sysHostEl.innerText = d.hostname;
            }
        })
        .catch(err => console.error(err));
}

export function loadProcesses() {
    fetch('/api/processes?limit=15')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                const tbody = document.getElementById('process-table-body');
                if (!tbody) return;
                tbody.innerHTML = '';
                res.data.forEach(p => {
                    const tr = document.createElement('tr');
                    
                    // Determine badge state
                    let stateClass = 'badge-sleeping';
                    let stateLabel = 'Sleeping';
                    if (p.status === 'R') { stateClass = 'badge-running'; stateLabel = 'Running'; }
                    if (p.status === 'T') { stateClass = 'badge-stopped'; stateLabel = 'Stopped'; }

                    tr.innerHTML = `
                        <td style="font-family:var(--font-code); font-weight:600;">${p.pid}</td>
                        <td style="font-family:var(--font-code); max-width:240px; overflow:hidden; text-overflow:ellipsis;" title="${p.name}">${p.name}</td>
                        <td>${p.cpu.toFixed(1)}%</td>
                        <td>${p.memory.toFixed(1)}%</td>
                        <td><span class="badge ${stateClass}">${stateLabel}</span></td>
                        <td style="text-align:right;">
                            <button class="btn-icon" onclick="killProcess(${p.pid})" title="Kill process">
                                <i class="fa-solid fa-skull"></i>
                            </button>
                        </td>
                    `;
                    tbody.appendChild(tr);
                });
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

export function startStatsPolling() {
    if (!sysStatsInterval) {
        loadSystemStats();
        loadProcesses();
        sysStatsInterval = setInterval(() => {
            loadSystemStats();
            loadProcesses();
        }, 3000);
    }
}

export function stopStatsPolling() {
    if (sysStatsInterval) {
        clearInterval(sysStatsInterval);
        sysStatsInterval = null;
    }
}
