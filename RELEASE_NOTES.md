# 🚀 ZenoPanel Release Notes

Daftar rilis resmi fitur, perbaikan, dan peningkatan teknologi pada platform ZenoPanel.

---

## 🌟 Versi v1.8.1 (Rilis Terbaru)

Rilis **v1.8.1** membawa peningkatan besar pada ketahanan dan stabilitas sistem melalui implementasi error handling terpusat di seluruh lapisan rute API, pembersihan komponen tidak aktif, serta upgrade mesin skrip internal ZSL ke versi terbaru.

---

### 🛡️ 1. Error Handling Terpusat via Try/Catch di Semua Rute API

Seluruh endpoint API ZenoPanel kini dibungkus dengan blok `try/catch` ZSL yang memberikan perlindungan menyeluruh terhadap kegagalan runtime. Sebelumnya, kegagalan operasi sistem (I/O disk penuh, koneksi DB terputus, kontainer tidak ditemukan, dsb.) dapat menyebabkan request hang tanpa respons atau mengembalikan 500 mentah ke browser.

**Rute yang diupgrade:**
- **`database.zl`** — install-server, toggle-remote, toggle-pool, create, delete
- **`box.zl`** — seluruh lifecycle manajemen kontainer & compose
- **`files.zl`** — semua operasi file manager (read, write, delete, rename, upload, unzip)
- **`proxy.zl`** — manajemen aturan reverse proxy
- **`cron.zl`** — penjadwalan & sinkronisasi cron job
- **`firewall.zl`** — status & aturan firewall, lockdown mode
- **`system.zl`** — info sistem, proses, kill, update panel
- **`containers.zl`** — seluruh lifecycle kontainer, volume, network, dan live migration
- **`machine.zl`** — VM MicroVM (CRUD, ISO, snapshot, live migration)
- **`managed.zl`** — proses native (add, edit, start/stop/restart, git sync, webhook)
- **`migrate.zl`** — preflight check, progress stream
- **`services.zl`** — kontrol service OS (Docker, Nginx, MySQL, PostgreSQL)
- **`settings.zl`** — semua pengaturan admin (backup, security/WAF, log rotation, registry)
- **`users.zl`** — manajemen akun pengguna
- **`terminal.zl`** — eksekusi perintah shell interaktif
- **`auth.zl`** — login, logout, rate-limit, JWT

Pola yang digunakan:
```yaml
try: {
    run: { # Logika eksekusi utama }
    catch: {
        http.bad_request: { success: false, message: "Pesan error: ${error}" }
    }
}
```

---

### 🗑️ 2. Penghapusan Modul Websites

Modul **Websites** yang sebelumnya tidak aktif digunakan kini telah dibersihkan sepenuhnya dari codebase:
- Dihapus: `zsrc/routes/websites.zl`
- Dihapus: `views/partials/tab_websites.blade.zl`
- Dihapus: `public/js/websites.js`
- Dihapus: referensi navigasi dari `sidebar.blade.zl`, `views.zl`, `navigation.js`, dan `app.js`

---

### ⚙️ 3. Upgrade ZenoCore & Ekosistem ZSL ke v0.2.3

ZenoPanel kini menggunakan `zenocore 0.2.3` yang menyertakan slot `try/catch` secara native di mesin skrip ZSL. Perubahan ini memindahkan implementasi try/catch dari layer aplikasi ZenoPanel ke dalam library inti, sehingga tersedia untuk seluruh proyek yang menggunakan ZenoCore:

| Crate | Sebelum | Sesudah |
|---|---|---|
| `zenocore` | 0.2.2 | **0.2.3** (+ slot `try/catch`) |
| `zeno-std` | 0.2.2 | **0.2.3** |
| `zeno-blade` | 0.2.2 | **0.2.3** |
| `zeno-apidoc` | 0.2.2 | **0.2.3** |
| `zenoengine` | 0.2.2 | **0.2.3** |

Implementasi `try` duplikat di `src/slots/util.rs` ZenoPanel dihapus karena kini sudah tersedia dari upstream zenocore.

---

### 📝 4. Peningkatan Penanganan Editor Monaco

- **Pembersihan Instance Otomatis**: Memperbaiki masalah inisialisasi ulang editor dengan dispose pada instance lama jika elemen kontainer kosong di DOM.
- **Auto-Layout Resizing**: Trigger layout otomatis saat pengguna berpindah tab, mencegah masalah ukuran editor yang menyusut.
- **Optimasi Layout**: Penyesuaian `min-height` pada area kerja editor Compose dan konsol output.

---

### 🗄️ 5. Perbaikan Logika Pembuatan YAML Database & Sanitasi Password

- **Refaktor Variabel YAML**: Memperbaiki pembuatan Docker Compose YAML pada PostgreSQL & MySQL agar tidak menggunakan variabel self-referential.
- **Peningkatan Sintaks YAML**: Nilai environment password kini dibungkus dengan tanda kutip tunggal (`'`) untuk kepatuhan standar YAML.
- **Sanitasi Password Generator**: Membatasi karakter password hanya pada alfanumerik untuk menghindari kegagalan parsing karakter khusus.

---

### 📦 6. Informasi Distribusi

| Artefak | Ukuran | Keterangan |
|---|---|---|
| `zenopanel-v1.8.1.tar.gz` | ~26 MB | Static binary Linux (x86_64-musl) |
| `zenopanel-windows-v1.8.1.zip` | ~29 MB | WSL2 distro + launcher.exe untuk Windows |



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
