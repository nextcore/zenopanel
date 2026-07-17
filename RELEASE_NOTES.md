# Release Notes - ZenoPanel v1.5.20

Rilis **v1.5.20** membawa pembaruan besar pada sistem instalasi dan launcher Windows. Kami melakukan perombakan penuh dari Go Launcher ke Native Zig Launcher yang jauh lebih ringan, menghadirkan distro ZenoOS berbasis Alpine Linux teroptimasi untuk WSL 2, serta meningkatkan kompatibilitas penuh untuk sistem operasi Windows 10.

---

## 💻 Cara Pemasangan di Windows
1. Unduh berkas `zenopanel-windows-v1.5.20.zip`.
2. Ekstrak berkas tersebut ke folder pilihan Anda.
3. Jalankan `zenopanel-launcher.exe` untuk mengimpor distro ZenoOS dan menyalakan panel secara otomatis.
4. Buka dasbor ZenoPanel di browser Anda melalui tautan: [http://localhost:3001](http://localhost:3001)

---

## 🚀 Apa yang Baru (New Features)

### 📦 Instalasi Offline 100% (Pre-Baked Distro)
*   **Bebas Isu Jaringan WSL 2**: Guna mencegah kegagalan instalasi di Windows 10 akibat rusaknya resolusi DNS bawaan WSL 2 (NAT mode), paket ZenoPanel core (HTML, JS, Rust binary `zeno`, dan konfigurasi `.env` default) kini **di-bake secara langsung** ke dalam distro `zenoos-v1.5.20.tar.gz` saat proses pengemasan (`package_wsl.sh`).
*   **Zero-Network Install**: Begitu launcher mengimpor distro `zenoos`, ZenoPanel sudah terpasang 100% secara instan dan dapat langsung dinyalakan tanpa memerlukan koneksi internet/DNS atau dependensi eksternal di dalam WSL 2.

### ⚡ Launcher Windows Asli (Native Zig Launcher)
*   **Ukuran Super Ringan**: Launcher di-rewrite sepenuhnya menggunakan bahasa Zig, memotong ukuran berkas executable secara signifikan menjadi hanya **~200 KB** (sebelumnya menggunakan Go).
*   **Perintah Layanan Baru**: Menambahkan argumen `--stop` untuk mematikan layanan distro ZenoOS secara bersih dan aman langsung dari terminal/command prompt.
*   **Penyempurnaan Penanganan Error**: Integrasi dialog informasi kesalahan WSL 2 yang lebih detail dan interaktif jika sistem belum mengaktifkan fitur virtualisasi.
*   **Autostart Pintar**: Dukungan argumen `--silent` dan notifikasi sistem (Toast Balloon Notification) saat background service berhasil dinyalakan.

### 🌐 Distro ZenoOS & Otomatisasi WSL 2 Packaging
*   **Rebranding ZenoOS**: Distro WSL 2 dikustomisasi berbasis Alpine Linux terkini, lengkap dengan penyesuaian `/etc/motd`, `/etc/issue`, dan `/etc/os-release` khas ZenoOS.
*   **Script WSL Packager Terpadu**: Penambahan berkas `package_wsl.sh` untuk melakukan otomasi ekstraksi Alpine rootfs, pengintegrasian core ZenoPanel ke `/opt/zenopanel`, kompilasi launcher Zig, dan pengemasan ZIP secara lokal dalam satu langkah.

### 🛠️ Script Rilis Otomatis (`release.sh`)
*   **Automated Release**: Penambahan berkas `release.sh` interaktif untuk mengotomatiskan proses upload aset rilis ke GitHub via GitHub CLI (`gh`), termasuk integrasi opsional dengan AI Google Gemini untuk menghasilkan catatan rilis.

---

## 🔧 Perbaikan & Peningkatan Stabilitas (Bug Fixes & Hardening)

*   **Dukungan Penuh Windows 10**: Menambahkan fungsi pengecekan build version dinamis (`RtlGetVersion` dari `ntdll.dll`). Launcher kini mendeteksi jika pengguna menggunakan Windows 10 (Build < 22621) dan secara otomatis melewati (*bypass*) konfigurasi eksperimental `.wslconfig` (seperti `virtFSMounting`, `networkingMode=mirrored`, `dnsTunneling`) agar WSL 2 dapat berjalan normal dan stabil tanpa error ketidakcocokan fitur.
*   **Penyederhanaan Launcher Container (`/usr/local/bin/zenopanel`)**: Skrip launcher container disederhanakan dengan menghapus pengetesan internet dan pengunduhan dinamis `install.sh` dari luar. Skrip kini hanya memverifikasi dependensi dasar (`iptables`/`iproute2`) dan langsung mengeksekusi `./zeno` dari direktori `/opt/zenopanel` yang sudah terisi.
*   **Perbaikan POSIX Shell di `install.sh`**: Memperbaiki sintaks parser parameter di `install.sh` dari bash-ism `[[ ... ]]` ke standar POSIX `[ ... ]` agar kompatibel dengan shell `/bin/sh` (`ash`/`dash`) bawaan Alpine Linux di server VPS standalone.


---

## 📦 Aset Rilis (Release Assets)
*   **`zenopanel-windows-v1.5.20.zip`**: Paket client launcher Windows (`zenopanel-launcher.exe`).
*   **`zenoos-v1.5.20.tar.gz`**: Tarball distro dasar ZenoOS berbasis Alpine Linux.
*   **`zenopanel-v1.5.20.tar.gz`**: Paket inti aplikasi ZenoPanel standalone untuk Linux.
