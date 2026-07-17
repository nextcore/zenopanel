# Release Notes - ZenoPanel v1.5.22

Rilis **v1.5.22** berfokus pada penyelesaian permanen masalah terminal PTY (kemampuan fallback shell ke `sh`) di lingkungan yang tidak memiliki `bash` seperti Alpine Linux (WSL), serta optimasi pemrosesan perintah sistem dan pembaruan otomatis latar belakang.

---

## 🔧 Perbaikan & Peningkatan Stabilitas (Bug Fixes & Improvements)

### 🐚 Perbaikan Permanen Fallback Terminal PTY ke `/bin/sh`
*   **Penyelesaian Error PTY**: Mengatasi masalah error `"Failed to spawn command in PTY: unable to spawn bash..."` yang masih muncul pada beberapa sistem.
*   **Logika Fallback Tanpa Syarat**: Memperbaiki mekanisme pengecekan error sebelumnya yang menggunakan `downcast_ref::<std::io::Error>()`. Karena `portable-pty` mengemas error ke dalam string dinamis (`anyhow::Error`), pencocokan tipe error sebelumnya gagal dievaluasi. Sekarang, sistem secara otomatis beralih ke shell `/bin/sh` jika `bash` gagal dipicu di lingkungan host tanpa memedulikan jenis error spesifik.
*   **Peningkatan Keandalan system.exec**: Mengoptimalkan fungsi eksekusi perintah custom di `system.exec` (`src/slots/system.rs`) agar langsung mencoba shell fallback `sh` jika `bash` tidak merespons atau tidak ditemukan.
*   **Pembaruan Latar Belakang Mandiri (Background Updater)**: Memperbaiki updater otomatis ZenoPanel agar memiliki fallback perintah pembaruan menggunakan `sh` ketika sistem operasi dasar (seperti Alpine minrootfs) tidak menyediakan shell `bash`.

---

## 📦 Aset Rilis (Release Assets)
*   **`zenopanel-windows-v1.5.22.zip`**: Berkas client launcher Windows (`zenopanel-launcher.exe`) versi v1.5.22.
*   **`zenoos-v1.5.22.tar.gz`**: Tarball distro dasar ZenoOS berbasis Alpine Linux v3.24 dengan perbaikan shell fallback.
*   **`zenopanel-v1.5.22.tar.gz`**: Paket inti aplikasi ZenoPanel standalone untuk Linux.
