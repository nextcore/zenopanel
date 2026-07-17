# Release Notes - ZenoPanel v1.5.21

Rilis **v1.5.21** berfokus pada peningkatan kompatibilitas terminal PTY di lingkungan Alpine Linux (WSL) dan menghadirkan antarmuka grafis (GUI) kontrol baru berbasis Windows API native yang sangat ringan dan efisien untuk komputer berspesifikasi rendah (*low-RAM devices*).

---

## 🚀 Apa yang Baru (New Features)

### 🖥️ Windows Launcher GUI Native Baru (Ultra Lightweight UI)
*   **Target Device Low-RAM**: Menggantikan alur dialog MessageBox beruntun dengan jendela aplikasi (Control Center) berbasis Win32 API native. Aplikasi berjalan super ringan dengan penggunaan memori hanya sekitar **1.5 s.d. 3 MB RAM** (bebas dari overhead browser engine seperti WebView atau rendering engine berat seperti OpenGL/Flutter).
*   **Desain Modern & Dark Mode**: Menggunakan visual styles sistem terbaru Windows dengan dukungan *Immersive Dark Mode* (title bar gelap) pada Windows 10/11 dan latar belakang bertema dark slate (`#0f172a`).
*   **Pemantau Status Real-Time**: Mengintegrasikan *Background Thread Status Monitor* yang secara dinamis mendeteksi dan memperbarui label status ZenoPanel secara instan:
    *   `Aktif` (Teks Hijau) jika port 3001 merespons.
    *   `Tidak Aktif` (Teks Abu-abu) jika mati.
    *   `Memulai...` / `Menghentikan...` (Teks Biru Muda) saat proses berjalan.
*   **Tombol Kontrol Terpadu**:
    *   **Buka Dashboard**: Menyalakan WSL (jika mati) dan membuka browser dasbor secara otomatis.
    *   **Matikan Layanan (Stop WSL)**: Menghentikan VM distro WSL `zenopanel` secara bersih.
    *   **Autostart Dinamis**: Tombol interaktif yang otomatis mendeteksi status startup Windows dan memungkinkan pengguna untuk mengaktifkan/menonaktifkan autostart dengan sekali klik.
    *   **Copot ZenoPanel**: Akses cepat untuk melakukan uninstalasi total.

---

## 🔧 Perbaikan & Peningkatan Stabilitas (Bug Fixes)

### 🐚 Fallback Terminal PTY ke `/bin/sh` (Penyelesaian Isu Alpine Linux)
*   **Perbaikan Spawn Terminal**: Menyelesaikan error terminal `"Failed to spawn command in PTY: unable to spawn bash because it doesn't exist and was not found in PATH"` saat membuka terminal di sistem ZenoPanel berbasis WSL Alpine Linux.
*   **Mekanisme Fallback Pintar**: Mengubah logika interactive terminal (`src/main.rs`) dan eksekusi perintah custom (`src/slots/system.rs`) agar mencoba menjalankan `bash` terlebih dahulu, lalu secara otomatis beralih (*fallback*) ke shell bawaan **`sh`** jika `bash` tidak terpasang di sistem (seperti pada Alpine minrootfs).
*   **Penanganan Error Aman**: Menggunakan metode *downcasting* error `anyhow` ke `std::io::Error` untuk mengidentifikasi error `NotFound` secara aman pada level bahasa pemrograman Rust sebelum memicu fallback shell.

---

## 📦 Aset Rilis (Release Assets)
*   **`zenopanel-windows-v1.5.21.zip`**: Berkas client launcher Windows (`zenopanel-launcher.exe`).
*   **`zenoos-v1.5.21.tar.gz`**: Tarball distro dasar ZenoOS berbasis Alpine Linux v3.24.
*   **`zenopanel-v1.5.21.tar.gz`**: Paket inti aplikasi ZenoPanel standalone untuk Linux.
