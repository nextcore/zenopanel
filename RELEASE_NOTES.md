# ZenoPanel v1.9.1 Release Notes 🚀

**Release Date:** August 7, 2026  
**Tag:** `v1.9.1`  
**Distribution Bundle:** `zenopanel-v1.9.1.tar.gz`

---

## 🌟 Major Feature: Container Healthcheck & Volume Enhancements

ZenoPanel v1.9.1 menghadirkan peningkatan pada engine kontainer Zeno Box, kompatibilitas Docker Compose, serta sistem keamanan Web Application Firewall (WAF).

### 🚀 Key Highlights & Capabilities:

#### 1. Dukungan Healthcheck Kontainer (Zeno Box Compose)
- Mendukung opsi pengujian kesehatan (`healthcheck`) pada definisi servis Compose.
- Engine akan menunggu status kontainer menjadi sehat dengan menjalankan pengujian perintah (`CMD`, `CMD-SHELL`, atau perintah langsung) di dalam kontainer via `runc exec`.
- Menggunakan skema *retry* otomatis (default hingga 10 kali percobaan dengan jeda 2 detik) sebelum memberikan peringatan timeout jika kontainer tidak kunjung sehat.

#### 2. Kustomisasi & Volume Eksternal pada Compose
- Mendukung konfigurasi volume tingkat atas (*top-level* `volumes`) dengan parameter `external: true` atau penamaan kustom (`name`).
- Jika dikonfigurasi sebagai volume eksternal/kustom, engine tidak akan menambahkan prefiks nama proyek (`project_name_`), memudahkan berbagi volume antar-servis dan integrasi data yang sudah ada di host.

#### 3. Auto-Populate Named Volume dari Image Data
- Otomatis menginisialisasi isi *named volume* yang masih kosong menggunakan data bawaan yang ada pada direktori *rootfs* gambar (*image data*).
- Meniru perilaku native Docker untuk mencegah hilangnya file konfigurasi default saat pertama kali kontainer dipasang dengan volume baru.

---

## 🛡️ WAF Auto-Expiring Temporary IP Blocks
- Fitur keamanan login brute-force diperbarui dengan pembatasan IP sementara selama **5 menit**.
- IP yang diblokir otomatis oleh sistem karena melebihi batas kegagalan login akan dihapus dari daftar blokir WAF setelah masa kedaluwarsa habis saat dilakukan pemeriksaan berikutnya.

---

## 🛠️ API & Engine Changes

### Slot Mesin Baru & Logika yang Diperbarui:
- **`src/slots/zeno_box/compose.rs`**:
  - Implementasi logika pengecekan healthcheck menggunakan `runc exec`.
  - Penyesuaian pemetaan volume eksternal tanpa prefiks proyek.
- **`src/slots/zeno_box/container.rs`**:
  - Fungsi `copy_dir_all` dan `is_dir_empty` untuk menyalin konten awal image ke named volume.
- **`src/slots/auth.rs`**:
  - Penambahan mekanisme auto-unblock pada WAF jika masa berlaku blokir brute-force telah habis.

---

## 🎨 Launcher Updates
- Sinkronisasi versi default launcher native (`launcher/main.zig`) dan skrip PowerShell (`launcher/zenopanel.ps1`) ke versi **v1.9.1** untuk kestabilan distribusi.
