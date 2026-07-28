# 🚀 ZenoPanel Release Notes

Daftar rilis resmi fitur, perbaikan, dan peningkatan teknologi pada platform ZenoPanel.

---

## 🌟 Versi v1.8.1 (Rilis Terbaru)

Rilis **v1.8.1** berfokus pada refaktor kode, perbaikan penanganan editor Monaco, optimalisasi pembuatan YAML untuk database, serta pembersihan dependensi:

### 📝 1. Peningkatan Penanganan Editor Monaco
Penyempurnaan integrasi editor Monaco pada panel editor Compose dan SQL untuk pengalaman pengodean yang lebih stabil:
- **Pembersihan Instance Otomatis**: Memperbaiki masalah inisialisasi ulang editor dengan melakukan dispose pada instance lama jika elemen kontainer kosong di DOM.
- **Auto-Layout Resizing**: Penambahan trigger layout secara otomatis setelah pengguna berpindah tab, mencegah masalah visual di mana ukuran editor tidak pas atau menyusut.
- **Optimasi Layout**: Penyesuaian tinggi minimal (`min-height`) pada area kerja editor Compose dan konsol output agar lebih responsif di berbagai resolusi layar.

### 🗄️ 2. Perbaikan Logika Pembuatan YAML Database & Sanitasi Password
Mengatasi masalah parsing dan kompatibilitas pada modul manajemen database:
- **Refaktor Variabel YAML**: Memperbaiki logika pembuatan Docker Compose YAML pada database PostgreSQL & MySQL dengan menghindari penggunaan variabel yang merujuk pada diri sendiri (*self-referential variables*). Kode kini menggunakan penggabungan variabel terpisah yang lebih bersih.
- **Peningkatan Sintaks YAML**: Nilai environment password kini dibungkus menggunakan tanda kutip tunggal (`'`) untuk memastikan kepatuhan terhadap standar format YAML.
- **Sanitasi Password Generator**: Menyederhanakan pembuatan password aman pada `generateSecurePassword` dengan membatasi karakter hanya pada alfanumerik (`a-z`, `A-Z`, `0-9`) untuk menghindari kegagalan parsing karakter khusus pada shell atau file konfigurasi.

### 📦 3. Pembaruan Dependensi & Sistem Build
- **Transisi ke Crates.io**: Mengupdate dependensi proyek untuk menggunakan pustaka `zenocore` dan paket `zeno-*` versi `v0.2.2` langsung dari crates.io resmi, serta menghapus patch path lokal yang tidak lagi diperlukan.

---

## 🌟 Versi v1.8.0

Rilis **v1.8.0** menghadirkan peningkatan signifikan pada performa dan keandalan interaksi shell modul **Zeno Machine** (MicroVM berbasis Cloud-Hypervisor & Rust):

### ⌨️ 1. Perbaikan Input & Focus Console Zeno Machine
Kami memperbaiki masalah input pada **Web Serial Console (xterm.js)** agar interaksi dengan terminal microVM menjadi lancar dan mulus:
- **Auto Keyboard Focus**: Terminal konsol kini secara otomatis mendapatkan fokus keyboard sesaat setelah modal dibuka. Ini menghilangkan kebutuhan klik manual tambahan untuk mulai mengetik.
- **Click-to-Focus Handler**: Menambahkan listener klik pada kontainer terminal sehingga pengguna dapat dengan mudah mengembalikan fokus ketikan setelah berinteraksi dengan bagian lain dari antarmuka pengguna panel.

### 📡 2. Implementasi Local Echo Fallback pada Serial TTY
Mengatasi masalah input tidak tampil (karakter ketikan tidak muncul secara real-time di layar) saat mengoperasikan VM minimal (seperti AlmaLinux / Alpine direct kernel boot yang tidak menjalankan getty controller penuh di `ttyS0`):
- **Local Echo Emulator**: xterm.js kini memantulkan karakter alphanumeric & simbol dasar secara lokal secara real-time ke layar ketika diketik, tanpa mengandalkan driver echo TTY dari guest OS.
- **Backspace & Return Handlers**: Mengimplementasikan feedback visual penghapusan karakter (`\b \b`) untuk tombol Backspace, serta line break (`\r\n`) untuk tombol Enter secara lokal agar selaras dengan eksekusi di latar belakang.

### 📦 3. Sinkronisasi Build v1.8.0
- Penyelarasan versi sistem di `Cargo.toml`, `install.sh`, dan sidebar dashboard utama secara otomatis melalui alat kompilasi interaktif.
- Pengemasan paket distribusi statis `zenopanel-v1.8.0.tar.gz` dengan optimasi pemotongan debug symbol (stripped binary) sebesar 26MB.

---
