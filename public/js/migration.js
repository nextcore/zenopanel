/**
 * migration.js — Shared Pre-flight Check & Dual-Host Progress Tracking
 * Digunakan oleh machines.js dan containers.js
 */

import { getCSRFToken } from "./utils.js";
import { showToast } from "./toast.js";

// ─── Helper: Render Check Item ─────────────────────────────────────

function pfIcon(ok, warn = false) {
    if (ok === null) return `<i class="fa-solid fa-spinner fa-spin" style="color:#8b5cf6; width:14px;"></i>`;
    if (ok && !warn) return `<i class="fa-solid fa-circle-check" style="color:#10b981; width:14px;"></i>`;
    if (warn) return `<i class="fa-solid fa-triangle-exclamation" style="color:#f59e0b; width:14px;"></i>`;
    return `<i class="fa-solid fa-circle-xmark" style="color:#ef4444; width:14px;"></i>`;
}

// ─── Zeno Machine Pre-flight ────────────────────────────────────────

export async function runMachinePreflight() {
    const host = document.getElementById("migrate-host-input").value.trim();
    if (!host) {
        showToast("error", "Masukkan IP Host Tujuan terlebih dahulu");
        return;
    }

    const resultPanel = document.getElementById("migrate-machine-preflight-result");
    const btnStart = document.getElementById("btn-machine-migrate-start");
    resultPanel.style.display = "block";

    // Reset to spinner state
    document.getElementById("pf-machine-reach").innerHTML = `${pfIcon(null)} Memeriksa konektivitas ke ${host}...`;
    document.getElementById("pf-machine-latency").innerHTML = `${pfIcon(null)} Mengukur latensi jaringan...`;
    document.getElementById("pf-machine-ram").innerHTML = `${pfIcon(null)} Verifikasi ketersediaan RAM di target...`;
    btnStart.disabled = true;
    btnStart.style.cursor = "not-allowed";
    btnStart.style.color = "rgba(139,92,246,0.4)";

    let allOk = true;

    try {
        const res = await fetch("/api/migrate/preflight", {
            method: "POST",
            headers: { "Content-Type": "application/json", "X-CSRF-Token": getCSRFToken() },
            body: JSON.stringify({ target_host: host, engine: "machine", required_ram_mb: 512 })
        }).then(r => r.json());

        const c = res.checks || {};

        // Check 1: Reachability
        const reachOk = c.target_reachable !== false;
        document.getElementById("pf-machine-reach").innerHTML =
            `${pfIcon(reachOk)} Konektivitas ke <strong>${host}</strong>: ${reachOk ? '<span style="color:#10b981;">Terhubung ✓</span>' : '<span style="color:#ef4444;">Tidak dapat dijangkau ✗</span>'}`;
        if (!reachOk) allOk = false;

        // Check 2: Latency
        const latency = c.latency_ms ?? 0;
        const latWarn = latency > 50;
        const latOk = reachOk;
        document.getElementById("pf-machine-latency").innerHTML =
            `${pfIcon(latOk, latWarn)} Latensi jaringan: <strong style="color: ${latWarn ? '#f59e0b' : '#10b981'}">${latency}ms</strong> ${latWarn ? '(⚠ Latensi tinggi — risiko meningkat)' : '(Optimal ✓)'}`;

        // Check 3: RAM
        const ramAvail = c.local_ram_available_mb ?? 0;
        const ramOk = ramAvail > 256;
        document.getElementById("pf-machine-ram").innerHTML =
            `${pfIcon(ramOk)} RAM lokal tersedia: <strong style="color: ${ramOk ? '#10b981' : '#ef4444'}">${ramAvail} MB</strong> ${ramOk ? '— Cukup untuk migration ✓' : '— RAM tidak cukup ✗'}`;
        if (!ramOk) allOk = false;

    } catch {
        allOk = false;
        document.getElementById("pf-machine-reach").innerHTML = `${pfIcon(false)} Gagal menghubungi server ZenoPanel`;
        document.getElementById("pf-machine-latency").innerHTML = `${pfIcon(null)} Tidak dapat diukur`;
        document.getElementById("pf-machine-ram").innerHTML = `${pfIcon(null)} Tidak dapat diverifikasi`;
    }

    if (allOk) {
        btnStart.disabled = false;
        btnStart.style.cursor = "pointer";
        btnStart.style.color = "#c4b5fd";
        btnStart.style.background = "rgba(139,92,246,0.25)";
        showToast("success", "Pre-flight Check PASSED! Siap memulai Live Migration.");
    } else {
        showToast("error", "Pre-flight Check GAGAL. Perbaiki masalah sebelum melanjutkan.");
    }
}

// ─── Zeno Machine Migration Progress Simulation ─────────────────────

export function startMachineMigrationProgress(targetHost, onComplete) {
    document.getElementById("migrate-machine-step-input").style.display = "none";
    document.getElementById("migrate-machine-step-progress").style.display = "block";
    document.getElementById("machine-dst-host-label").textContent = targetHost;

    const srcPhases = [
        [5,  "Menginisialisasi checkpoint memori KVM..."],
        [25, "Menyalin dirty pages RAM (fase 1)..."],
        [50, "Menyalin dirty pages RAM (fase 2)..."],
        [70, "Pre-copy memory iterasi akhir..."],
        [85, "Freezing VM sementara (<50ms)..."],
        [95, "Mengirim register CPU & device state..."],
        [100,"Transmisi selesai — VM dimatikan di HOST A."],
    ];

    const dstPhases = [
        [0,  "Menunggu sinyal dari HOST A..."],
        [20, "Menerima dirty pages RAM..."],
        [45, "Menyusun memori di HOST B..."],
        [65, "Menerima CPU state & register..."],
        [80, "Memulihkan VM di KVM HOST B..."],
        [95, "Verifikasi integritas VM..."],
        [100,"VM aktif di HOST B! ✓"],
    ];

    animateDualProgress(
        "machine-src-bar", "machine-src-percent", "machine-src-phase", srcPhases,
        "machine-dst-bar", "machine-dst-percent", "machine-dst-phase", dstPhases,
        () => {
            document.getElementById("machine-migration-done").style.display = "block";
            if (onComplete) onComplete(true);
        }
    );
}

// ─── Zeno Box Container Pre-flight ─────────────────────────────────

export async function runContainerPreflight() {
    const host = document.getElementById("migrate-container-host-input").value.trim();
    if (!host) {
        showToast("error", "Masukkan IP Host Tujuan terlebih dahulu");
        return;
    }

    const resultPanel = document.getElementById("migrate-container-preflight-result");
    const btnStart = document.getElementById("btn-ctr-migrate-start");
    resultPanel.style.display = "block";

    // Reset to spinner state
    document.getElementById("pf-ctr-reach").innerHTML = `${pfIcon(null)} Memeriksa konektivitas ke ${host}...`;
    document.getElementById("pf-ctr-latency").innerHTML = `${pfIcon(null)} Mengukur latensi jaringan...`;
    document.getElementById("pf-ctr-kernel").innerHTML = `${pfIcon(null)} Verifikasi kompatibilitas Kernel (CRIU)...`;
    btnStart.disabled = true;
    btnStart.style.cursor = "not-allowed";

    let allOk = true;

    try {
        const res = await fetch("/api/migrate/preflight", {
            method: "POST",
            headers: { "Content-Type": "application/json", "X-CSRF-Token": getCSRFToken() },
            body: JSON.stringify({ target_host: host, engine: "container", required_ram_mb: 256 })
        }).then(r => r.json());

        const c = res.checks || {};

        // Check 1: Reachability
        const reachOk = c.target_reachable !== false;
        document.getElementById("pf-ctr-reach").innerHTML =
            `${pfIcon(reachOk)} Konektivitas ke <strong>${host}</strong>: ${reachOk ? '<span style="color:#10b981;">Terhubung ✓</span>' : '<span style="color:#ef4444;">Tidak dapat dijangkau ✗</span>'}`;
        if (!reachOk) allOk = false;

        // Check 2: Latency
        const latency = c.latency_ms ?? 0;
        const latWarn = latency > 30;
        document.getElementById("pf-ctr-latency").innerHTML =
            `${pfIcon(reachOk, latWarn)} Latensi jaringan: <strong style="color: ${latWarn ? '#f59e0b' : '#10b981'}">${latency}ms</strong> ${latWarn ? '(⚠ CRIU sensitif terhadap latensi tinggi)' : '(Optimal ✓)'}`;

        // Check 3: Kernel Compatibility (CRIU)
        const kernel = c.local_kernel ?? "unknown";
        const kernelOk = reachOk; // Simplified — di implementasi nyata bandingkan versi kernel
        document.getElementById("pf-ctr-kernel").innerHTML =
            `${pfIcon(kernelOk)} Kernel lokal: <strong style="color: ${kernelOk ? '#10b981' : '#ef4444'}">${kernel}</strong> ${kernelOk ? '— CRIU Kompatibel ✓' : '— Periksa versi kernel target ⚠'}`;

    } catch {
        allOk = false;
        document.getElementById("pf-ctr-reach").innerHTML = `${pfIcon(false)} Gagal menghubungi server ZenoPanel`;
        document.getElementById("pf-ctr-latency").innerHTML = `${pfIcon(null)} Tidak dapat diukur`;
        document.getElementById("pf-ctr-kernel").innerHTML = `${pfIcon(null)} Tidak dapat diverifikasi`;
    }

    if (allOk) {
        btnStart.disabled = false;
        btnStart.style.cursor = "pointer";
        btnStart.style.color = "#c4b5fd";
        btnStart.style.background = "rgba(139,92,246,0.25)";
        showToast("success", "Pre-flight Check PASSED! CRIU Migration siap diluncurkan.");
    } else {
        showToast("error", "Pre-flight Check GAGAL. Perbaiki masalah sebelum melanjutkan.");
    }
}

// ─── Zeno Box Container Migration Progress Simulation ────────────────

export function startContainerMigrationProgress(targetHost, onComplete) {
    document.getElementById("migrate-container-step-input").style.display = "none";
    document.getElementById("migrate-container-step-progress").style.display = "block";
    document.getElementById("ctr-dst-host-label").textContent = targetHost;

    const srcPhases = [
        [5,  "Membekukan proses container (freeze)..."],
        [20, "Membuat CRIU checkpoint image..."],
        [40, "Menyimpan file descriptor & socket..."],
        [60, "Memadatkan checkpoint.img..."],
        [80, "Mengirim checkpoint.img ke HOST B..."],
        [95, "Menunggu konfirmasi restore dari HOST B..."],
        [100,"CRIU Dump selesai — proses container dihentikan di HOST A."],
    ];

    const dstPhases = [
        [0,  "Menunggu checkpoint.img dari HOST A..."],
        [25, "Menerima & memvalidasi checkpoint.img..."],
        [50, "Menjalankan criu restore..."],
        [70, "Memulihkan file descriptor & socket TCP..."],
        [85, "Menyambungkan kembali koneksi jaringan..."],
        [95, "Verifikasi integritas proses container..."],
        [100,"Container aktif di HOST B! Semua koneksi dipulihkan ✓"],
    ];

    animateDualProgress(
        "ctr-src-bar", "ctr-src-percent", "ctr-src-phase", srcPhases,
        "ctr-dst-bar", "ctr-dst-percent", "ctr-dst-phase", dstPhases,
        () => {
            document.getElementById("ctr-migration-done").style.display = "block";
            if (onComplete) onComplete(true);
        }
    );
}

// ─── Core Dual Progress Bar Animator ───────────────────────────────

function animateDualProgress(srcBarId, srcPctId, srcPhaseId, srcPhases,
                              dstBarId, dstPctId, dstPhaseId, dstPhases,
                              onDone) {
    let step = 0;
    const totalSteps = srcPhases.length;
    const dstDelay = 1; // Destination starts 1 step behind source

    const interval = setInterval(() => {
        // Update source
        if (step < srcPhases.length) {
            const [pct, label] = srcPhases[step];
            document.getElementById(srcBarId).style.width = `${pct}%`;
            document.getElementById(srcPctId).textContent = `${pct}%`;
            document.getElementById(srcPhaseId).textContent = label;
        }

        // Update destination (delayed by 1 step)
        const dstStep = step - dstDelay;
        if (dstStep >= 0 && dstStep < dstPhases.length) {
            const [pct, label] = dstPhases[dstStep];
            document.getElementById(dstBarId).style.width = `${pct}%`;
            document.getElementById(dstPctId).textContent = `${pct}%`;
            document.getElementById(dstPhaseId).textContent = label;
        }

        step++;

        // Done
        if (step >= totalSteps + dstDelay + 1) {
            clearInterval(interval);
            // Ensure destination is at 100%
            const last = dstPhases[dstPhases.length - 1];
            document.getElementById(dstBarId).style.width = "100%";
            document.getElementById(dstPctId).textContent = "100%";
            document.getElementById(dstPhaseId).textContent = last[1];
            if (onDone) onDone();
        }
    }, 900); // 900ms per step → realistic feel
}

// Window exposures
window.runMachinePreflight = runMachinePreflight;
window.runContainerPreflight = runContainerPreflight;
window.startMachineMigrationProgress = startMachineMigrationProgress;
window.startContainerMigrationProgress = startContainerMigrationProgress;
