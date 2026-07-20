# Release Notes — ZenoPanel v1.6.0

Rilis **v1.6.0** merupakan rilis fitur besar (*major feature release*) yang memperkenalkan **Zeno Machine** — engine virtualisasi MicroVM berbasis [Cloud-Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) yang ditulis 100% dalam Rust, serta sistem **Live Migration** lengkap dengan **Manual Approval Handshake**, **Pre-flight Compatibility Check**, dan **Dual-Host Progress Tracking** untuk kedua engine: Zeno Machine (KVM) dan Zeno Box (CRIU).

---

## 🚀 Fitur Baru (New Features)

### 🖥️ Zeno Machine — Cloud-Hypervisor MicroVM Engine
*   **Engine Virtualisasi Baru**: Memperkenalkan **Zeno Machine**, alternatif hypervisor ringan untuk menjalankan MicroVM Linux dan Windows menggunakan binary `cloud-hypervisor` (static/musl, Alpine-compatible).
*   **Manajemen VM Penuh via GUI**: Buat, jalankan, hentikan, dan hapus Zeno Machine langsung dari antarmuka ZenoPanel tanpa menyentuh terminal.
*   **Live Resize (Dynamic RAM & vCPU)**: Ubah alokasi RAM dan jumlah vCPU pada VM yang sedang berjalan secara *real-time* tanpa me-reboot (*hot-plug*).
*   **Statistik Dashboard Terintegrasi**: Panel ringkasan menampilkan jumlah total VM, VM aktif, total vCPU dan RAM yang terpakai.

### 🔄 Dual-Engine Live Migration
*   **Zeno Machine Live Migration** (KVM Memory Sync): Pindahkan MicroVM yang sedang berjalan antar server fisik secara *zero-downtime* dengan jeda handoff kurang dari 50ms.
*   **Zeno Box Container Live Migration** (CRIU Checkpoint/Restore): Pindahkan container `runc` yang sedang aktif beserta seluruh state proses, file descriptor, dan socket TCP dengan waktu transmisi kurang dari 200ms.
*   **Opsi Live Migration di Menu Manage Container**: Tombol *Live Migration (CRIU)* kini tersedia di dropdown Manage pada setiap baris container Zeno Box.

### 🛡️ Manual Approval Handshake System
*   **Approval di Server Tujuan**: Setiap permintaan Live Migration kini memerlukan persetujuan manual dari Administrator server tujuan melalui GUI sebelum transmisi dimulai — mencegah VM/container asing memenuhi RAM server secara tidak sah.
*   **Glowing Handshake Banner**: Banner notifikasi bercahaya muncul otomatis di dashboard server penerima dengan tombol **`[ Accept & Receive ]`** dan **`[ Reject ]`**.
*   **Tabel Approval**: Semua permintaan migrasi masuk (`pending`) dicatat di database lokal (`db_migration_requests`) dan dapat dikelola kapan saja.

### 🩺 Pre-flight Compatibility Check
*   **Cek Otomatis Sebelum Migrasi**: Sebelum tombol "Start Migration" dapat diklik, pengguna wajib menjalankan **Pre-flight Check** yang memverifikasi:
    *   Konektivitas jaringan ke IP host tujuan
    *   Latensi antar server (peringatan jika >50ms untuk Zeno Machine, >30ms untuk CRIU)
    *   Ketersediaan RAM lokal
    *   Kompatibilitas versi kernel Linux (khusus Zeno Box CRIU)
*   **Hasil Visual**: Setiap pemeriksaan ditampilkan dengan ikon ✓ / ✗ / ⚠ secara real-time di dalam panel preflight modal.

### 📊 Dual-Host Progress Tracking
*   **Progress Bar Dua Arah**: Saat Live Migration berjalan, modal menampilkan **dua progress bar sekaligus** — satu untuk HOST A (sumber) dan satu untuk HOST B (tujuan) — dengan label fase teknis yang diperbarui setiap langkah.
*   **Fase Teknis Deskriptif**: Mulai dari *"Menginisialisasi checkpoint memori KVM"* hingga *"VM aktif di HOST B"* untuk Zeno Machine, dan dari *"Freezing proses container"* hingga *"CRIU Migration Berhasil!"* untuk Zeno Box.
*   **Banner Status Akhir**: Setelah migrasi selesai, banner hijau (*Berhasil*) atau merah (*Gagal + Rollback Otomatis*) ditampilkan di dalam modal.

---

## 📦 Perubahan Pengemasan (Packaging)

*   **Binary Cloud-Hypervisor Disertakan Otomatis**: `compile.sh` kini mengunduh `cloud-hypervisor-static v42.0` secara otomatis setelah kompilasi selesai dan menyertakannya di dalam tarball distribusi di folder `bin/`. Binary ini merupakan static binary (musl) yang sepenuhnya kompatibel dengan Alpine Linux.
*   **Smart Caching**: Binary Cloud-Hypervisor di-cache lokal di folder `bin/` dan hanya diunduh ulang jika versi berubah.

---

## 📦 Aset Rilis (Release Assets)

*   **`zenopanel-v1.6.0.tar.gz`**: Paket inti ZenoPanel untuk Linux, sudah termasuk binary `bin/cloud-hypervisor` (static/musl).
*   **`zenoos-v1.6.0.tar.gz`**: Distro ZenoOS berbasis Alpine Linux 3.24 dengan ZenoPanel v1.6.0 dan Cloud-Hypervisor terintegrasi.
*   **`zenopanel-windows-v1.6.0.zip`**: Client launcher Windows (`zenopanel-launcher.exe`) + PowerShell installer untuk pengguna Windows via WSL2.
