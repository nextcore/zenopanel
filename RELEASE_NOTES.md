# 🚀 ZenoPanel Release Notes

Daftar rilis resmi fitur, perbaikan, dan peningkatan teknologi pada platform ZenoPanel.

---

## 🌟 Versi v1.7.13 (Rilis Terbaru)

Rilis **v1.7.13** berfokus pada penguatan stabilitas **sistem database dan koneksi pool**, perbaikan sintaksis **ZenoLang (zenocore)**, serta penanganan templat **Docker Compose** yang lebih tangguh untuk Named Volumes.

### 🗄️ 1. Perbaikan Deserialisasi YAML & Named Volumes
Kami membenahi kompatibilitas penanganan direktori data kustom maupun Named Volumes saat mendeploy engine database:
- **Pemisah Baris Eksplisit**: Menyelesaikan error YAML parser (`mapping values are not allowed in this context`) dengan memisahkan `${limits_yaml}` dan `${volumes_block}` secara tegas menggunakan baris baru (`\n`) pada templat biner. Ini mencegah hilangnya pembatas baris yang sebelumnya terpotong akibat *newline stripping* di ZenoLang.
- **Dukungan Penuh Volume Kustom**: Sukses mendeploy container MySQL 5.7/5.6 dengan Named Volumes (tanpa path `/`) langsung melalui antarmuka GUI.

### ⚙️ 2. Peningkatan Toleransi Parser ZenoLang
Untuk mengatasi batasan bawaan pada parser `zenocore v0.2`, kami menambahkan mekanisme penanganan sintaksis global di sisi ZenoPanel:
- **Trailing Delimiter Stripper**: Backend Rust kini secara otomatis menyaring tanda koma buntut (`,`) dan spasi di ujung tanda kurung kurawal pada parameter ZenoLang sebelum mengevaluasi variabel.
- **Penyempurnaan Blok Kondisional (`contains`)**: Mengubah penulisan parameter berisi koma pada blok `{ contains: ... }` menjadi string tunggal ter-quote, yang kemudian diuraikan secara dinamis di sisi Rust custom slot.

### 🔌 3. Integrasi Live Status & Connection Pool (ProxySQL / PgBouncer)
- **Status Database Presisi**: Halaman manajemen database kini menampilkan jumlah koneksi aktif, uptime server, dan versi mesin database secara real-time, menggantikan teks placeholder variabel mentah.
- **ProxySQL Toggling**: Memperbaiki konkatenasi string biner pada penyusunan berkas konfigurasi `proxysql.cnf` untuk kelancaran pengaktifan *connection pool* MySQL.

---

## 🌟 Versi v1.7.11

Rilis **v1.7.11** berfokus pada stabilitas, peningkatan pengalaman pengguna (*developer experience*), dan ketangguhan fitur virtualisasi pada **Zeno Machine (Cloud-Hypervisor MicroVM)**.

### 🖥️ 1. Web Serial Console (xterm.js & WebSockets)
Kami menggantikan penanganan konsol CLI lama dengan integrasi terminal penuh:
- **Real-Time Stream**: Menghubungkan terminal web secara asinkron ke soket serial asli milik microVM (`alpine-serial.sock`) menggunakan `xterm.js` dan WebSocket pipe.
- **Auto-Fit Layout**: Terminal otomatis menyesuaikan kolom dan baris jendela browser secara responsif.
- **Zero Memory Leak**: Stream I/O dikelola dengan aman menggunakan thread-safe async task di sisi backend Rust.

### 💾 2. Dynamic Disk Resizing (Offline & Live Online)
Sekarang Anda dapat memperbesar kapasitas penyimpanan Zeno Machine secara fleksibel:
- **Offline Resize**: Jika VM dalam keadaan mati (`stopped`), berkas `.img` di host akan otomatis diperluas secara instan (*sparse allocation*) saat booting sesuai kapasitas baru di database.
- **Live Online Hot-Plug**: Jika VM dalam keadaan menyala (`running`), ZenoPanel akan memperbesar kapasitas berkas host secara *live*, lalu mengirim perintah perluasan ke soket kontrol REST API Cloud-Hypervisor (`/vm.resize-disk`). Driver VirtIO di Guest OS akan langsung mendeteksi ruang ekstra tersebut secara real-time tanpa reboot.

### ⚙️ 3. Resilient ZenoLang String Interpolation & Scope Resolver
Perbaikan mesin templating ZenoLang untuk meminimalisasi kegagalan notifikasi:
- **JSON Response Interpolation**: Fungsi respons `http.ok` dan sejenisnya sekarang mengevaluasi template string secara rekursif sehingga mendukung placeholder `${name}` secara native.
- **Prefix-Agnostic Lookup**: Mekanisme pencarian scope di backend Rust kini otomatis mengenali variabel dengan atau tanpa prefix `$` (seperti `$name` vs `name`) di seluruh level objek datar maupun bersarang (*nested properties*).

### 🔧 4. Perbaikan Firmware UEFI & Booting
- Memperbarui mekanisme download otomatis firmware UEFI EDK2 (`ensure_firmware`) ke endpoint rilis terbaru yang valid (`CLOUDHV.fd` dari Cloud-Hypervisor), menyelesaikan masalah *404 Not Found* pada instalasi awal.

---

## 🛠️ Ringkasan Perubahan Teknis (Changelog v1.7.11)
- **[Feature]**: Implementasi modul terminal serial WebSocket di [src/machineman.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/machineman.rs).
- **[Feature]**: Logika auto-resize disk offline/online terintegrasi di loop konsolidator backend.
- **[Bug Fix]**: Memperbaiki loop rekonsiliasi tak terbatas (*infinite starting loop*) pada `start_machine` dengan mengganti validasi status string menjadi pengecekan PID proses aktif (`state.pid.is_some()`).
- **[Bug Fix]**: Memperbaiki resolusi string JSON response di [src/slots/mod.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/mod.rs) untuk mencegah pesan `undefined` di toast.
- **[UI/UX]**: Menambahkan input field ukuran penyimpanan (Disk GB) pada modal resize spesifikasi mesin.

---

## 📥 Cara Update ke v1.7.11
Cukup tarik pembaruan kode terbaru dan jalankan script kompilasi:
```bash
git pull origin main
./compile.sh
```

---

## 🚀 ZenoPanel Release Notes — v1.7.0

Kami dengan bangga mengumumkan rilis **ZenoPanel v1.7.0**! 

Versi ini membawa peningkatan besar pada fondasi *scripting & template engine* dengan mengintegrasikan **`zenocore v0.2.0`** resmi dari [crates.io](https://crates.io/crates/zenocore), menghadirkan **50+ slot standar baru**, serta memperkenalkan dukungan **Native C-ABI Dynamic Plugin System**.

### Sorotan Utama Rilis v1.7.0
- **Integrasi `zenocore v0.2.0` dari crates.io**: Bermigrasi ke paket modular resmi untuk jaminan stabilitas jangka panjang.
- **Suite 50+ Slot Bawaan (Standard Library)**: Akses penuh manipulasi string (`string.*`), matematika (`math.*`), array, dan operator logis kompleks.
- **Native Dynamic Plugin Engine (`plugin.load`)**: Kemampuan memuat modul ekstensi biner luar (.so/.dylib) secara runtime tanpa kompilasi ulang.
