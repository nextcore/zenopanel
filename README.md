# ZenoPanel

**ZenoPanel** adalah server management panel berbasis web yang dibangun di atas [Zeno Rust](https://github.com/nextcore/zeno-rs) — runtime bahasa scripting *ZenoLang* yang ditulis dengan Rust. Dirancang sebagai alternatif ringan dan self-hosted dari aaPanel atau 1Panel, ZenoPanel cocok bagi developer yang ingin mengontrol server mereka tanpa ketergantungan pada platform pihak ketiga.

> 🦀 **Dibangun dengan Rust.** Single binary, zero-dependency runtime, SQLite-first. Tidak butuh PHP, Node.js, atau Docker untuk jalan.

---

## ✨ Fitur Utama

### 🖥️ Process Manager (supervisord-like)
- Kelola proses background layaknya supervisord — tanpa perlu install supervisord
- Start, Stop, Restart proses langsung dari UI
- **Auto-restart** otomatis jika proses crash (dengan exponential backoff)
- Real-time log streaming per proses (stdout + stderr)
- Monitoring CPU dan RAM usage per proses
- Deteksi port yang sedang digunakan proses
- Isolasi environment variable — env ZenoPanel tidak mencemari proses yang dikelola
- Persistensi ke SQLite: konfigurasi proses tidak hilang saat server restart

### 🔀 Reverse Proxy & Load Balancing
- Route traffic berdasarkan **domain** dan **path prefix**
- Mendukung **alternative domain** (contoh: `www.` redirect ke apex)
- Strip path prefix sebelum diteruskan ke backend
- Toggle enable/disable rule tanpa hapus konfigurasi
- Terintegrasi dengan Process Manager: cek status proses sebelum forward request
- Tampilkan halaman error yang informatif jika backend offline
- Mendukung **dynamic port listeners** — rule proxy dapat mendengarkan di port non-standar
- **Least Connections Load Balancing**: Pembagian beban request cerdas ke target backend yang paling sedikit memegang koneksi aktif.
- **Active Health Checks**: Worker background berkala untuk memantau kesehatan target backend dan secara dinamis mengecualikan target yang offline.

### 🔒 SSL/TLS Otomatis & HTTP/2 ALPN
- **Self-signed certificate** di-generate otomatis on-demand per domain sebagai fallback instan.
- Protokol **HTTP/2 Multiplexing & ALPN** (`h2` dan `http/1.1`) didukung secara native untuk kecepatan akses optimal.
- Alur **ACME Let's Encrypt asli** menggunakan pustaka produksi `instant-acme` v0.8.5 dengan pembuatan CSR berbasis `rcgen`.
- **Pembaruan Sertifikat Otomatis Berbasis X.509**: Background auto-renewal worker (berjalan setiap 12 jam) yang melakukan parsing validitas sertifikat asli menggunakan `x509-parser` dan memicu pembaruan otomatis jika masa aktif tersisa kurang dari 30 hari.
- Sertifikat di-cache secara efisien di disk dan in-memory untuk meminimalkan beban handshake.
- Server HTTPS berjalan paralel di port terpisah (default: 8443).

### 👥 Multi-User & RBAC
- Manajemen user dengan tiga role: **Admin**, **Editor**, **Viewer**
- Autentikasi via **JWT** yang disimpan dalam cookie HttpOnly
- **CSRF protection** bawaan untuk semua POST/PUT/DELETE request
- Role gating di sisi server: Viewer tidak bisa melakukan mutasi apapun
- Admin-only access untuk Terminal dan Database console

### 🛡️ Security (WAF & Rate Limiting)
- **Web Application Firewall (WAF)**: Melindungi server dari serangan SQL Injection, XSS, Path Traversal, dan RCE.
- **Rate Limiting**: Membatasi jumlah request maksimum per IP dalam jangka waktu tertentu untuk mencegah abuse/DDoS.
- **Dedicated Security Menu**: Menu khusus (Admin-only) untuk memantau audit trail log dari request yang diblokir, mengaktifkan/menonaktifkan WAF, dan menyesuaikan konfigurasi Rate Limiter secara real-time.

### 🗃️ Database Console
- Eksekusi raw SQL query langsung dari UI (khusus Admin)
- Mendukung SQLite (default) — arsitektur DB manager bisa diperluas
- Koneksi internal database terpisah dari database aplikasi

### 📁 File Manager
- Browse direktori server dari browser
- Upload file via multipart form
- Buat folder dan file baru
- Hapus file dan direktori

### 💻 Interactive Terminal
- Akses shell server langsung dari browser (khusus Admin)

### 📊 Dashboard
- Monitoring resource server: CPU, RAM, Disk, Uptime
- Ringkasan proses aktif dan status proxy

---

## 🏗️ Arsitektur

ZenoPanel dibangun di atas stack Rust yang solid:

| Komponen | Teknologi |
|---|---|
| Web framework | [Axum](https://github.com/tokio-rs/axum) |
| Async runtime | [Tokio](https://tokio.rs/) |
| Database | [SQLx](https://github.com/launchbadge/sqlx) + SQLite |
| TLS & ALPN | [Rustls](https://github.com/rustls/rustls) + [tokio-rustls](https://github.com/tokio-rs/tls) |
| HTTP client (proxy) | [reqwest](https://github.com/seanmonstar/reqwest) |
| Routing engine | ZenoEngine (ZenoLang runtime) |
| Template engine | Zeno-Blade (Blade-style template) |
| Cert generation | [rcgen](https://github.com/rustls/rcgen) |
| ACME client | [instant-acme](https://github.com/jsha/instant-acme) v0.8.5 |
| X.509 parser | [x509-parser](https://github.com/rusticata/x509-parser) |

Route handling, template rendering, dan logika bisnis didefinisikan dalam file `.zl` (ZenoLang), sehingga logika panel bisa dimodifikasi tanpa recompile Rust.

---

## 🚀 Menjalankan ZenoPanel

### Prasyarat
- [Rust](https://rustup.rs/) (stable terbaru)
- Git
- OpenSSL (untuk helper pembentukan cert pengetesan lokal)

### Clone & Build

```bash
git clone https://github.com/username/zenopanel.git
cd zenopanel
cargo build --release
```

### Konfigurasi

Salin file environment example dan sesuaikan:

```bash
cp .env.example .env
```

Konfigurasi minimal di `.env`:

```env
APP_PORT=:3000
APP_TLS_PORT=:8443
DB_DRIVER=sqlite
DB_NAME=./zeno.db
DB_INTERNAL_NAME=./zeno_internal.db
JWT_SECRET=ganti_dengan_random_string_panjang
ADMIN_USERNAME=admin
ADMIN_PASSWORD=password_anda
```

### Jalankan

```bash
cargo run --release
```

Server akan berjalan di:
- HTTP: `http://localhost:3000`
- HTTPS: `https://localhost:8443`

Login di `http://localhost:3000/login` menggunakan credential yang dikonfigurasi di `.env`.

---

## 📁 Struktur Project

```
zenopanel/
├── src/
│   ├── main.rs          # Entry point, router, middleware
│   ├── procman.rs       # Process Manager (lifecycle, logging, monitoring)
│   ├── proxyman.rs      # Reverse Proxy & Load Balancer + Health checks
│   ├── sslman.rs        # SSL/TLS manager + ACME flow + X.509 auto-renewal
│   ├── auth.rs          # JWT generate & verify
│   ├── db.rs            # Database connection manager
│   └── slots/           # ZenoLang custom slots (HTTP, IO, DB, Auth, dll)
├── views/
│   └── partials/        # UI template per tab (Blade-style .zl)
├── zsrc/
│   └── main.zl          # Definisi route utama (ZenoLang)
├── public/              # Static assets (CSS, JS, gambar)
├── logs/                # Log file per proses yang dikelola
├── certs/               # SSL certificates per domain
└── .env.example         # Template konfigurasi
```

---

## 🔐 Keamanan

- JWT disimpan dalam cookie **HttpOnly** (tidak dapat diakses JavaScript)
- CSRF token wajib untuk semua request mutasi (configurable)
- Role-based access control di **sisi server** (bukan hanya UI)
- Environment variable ZenoPanel **diisolasi** agar tidak bocor ke proses yang dikelola
- Entrance path (URL login) dapat dikustomisasi untuk security-by-obscurity
- Menu keamanan terpisah untuk kontrol granular terhadap WAF & Rate Limiting serta log audit trail blocked requests

> 💡 **Catatan keamanan:** Interactive Terminal memberikan akses shell penuh — pastikan hanya Admin terpercaya yang memiliki akses. ACME flow untuk Let's Encrypt menggunakan self-signed certificate sebagai fallback instan.

---

## 🆚 Kenapa ZenoPanel? (Perspektif Developer Rust/Go)

aaPanel dan 1Panel dirancang untuk sysadmin yang mengelola server berbasis LAMP/LEMP stack. ZenoPanel dirancang untuk developer yang **membangun dan mendeploy aplikasinya sendiri** — terutama yang menggunakan Rust, Go, atau bahasa compiled lainnya di mana konsep "install PHP dulu" tidak relevan.

### Masalah nyata dengan aaPanel & 1Panel untuk developer Rust/Go:

| Pain Point | aaPanel / 1Panel | ZenoPanel |
|---|---|---|
| **Instalasi panel** | Butuh curl pipe ke bash, install ratusan MB dependensi (PHP, Nginx, MySQL, dll) | `cargo build` → satu binary. Selesai. |
| **Deploy app Rust/Go** | Upload binary manual, tulis config vhost Nginx, reload service, buat systemd unit sendiri | Isi form: command, cwd, env vars → tekan Start. Auto-restart kalau crash. |
| **Reverse proxy** | Edit file `/etc/nginx/conf.d/...`, test config, reload Nginx | Tambah rule di UI → langsung aktif, tidak ada file config yang disentuh |
| **Lihat log app** | SSH ke server, `journalctl -u` atau `tail -f` file log | Buka tab Logs di UI, real-time dari browser |
| **Environment variables** | Set di systemd unit file or `.env` yang dikelola manual | Kelola per-proses di UI, tersimpan di DB, tidak bocor antar proses |
| **Multi-user akses server** | Buat Linux user baru, atur sudo permissions, pasang SSH key | Tambah user di UI, assign role (Admin/Editor/Viewer), selesai |
| **Kustomisasi behavior panel** | Fork repo, modifikasi kode PHP/Go, rebuild | Tulis atau edit file `.zl` (ZenoLang), tidak perlu recompile |

### Untuk developer Rust/Go, ZenoPanel adalah tool yang "get out of your way":

- **Zero overhead.** Tidak ada Nginx, PHP-FPM, atau MySQL yang ikut terinstall. Server resource sepenuhnya untuk aplikasi kamu.
- **SQLite sebagai state.** Seluruh konfigurasi ada di satu file `.db`. Backup = copy satu file. Migrate = copy dua file (binary + db).
- **Codebase yang terbaca.** ~50KB Rust yang bisa kamu audit, fork, dan modifikasi sesuai kebutuhan spesifik kamu.
- **Proxy yang aware terhadap proses.** Rule proxy bisa di-link ke managed process — jika proses mati, proxy langsung tampilkan halaman error informatif alih-alih connection refused.
- **RBAC yang proper untuk tim kecil.** JWT + bcrypt + CSRF — bukan basic auth atau shared password di config file.

> aaPanel dan 1Panel adalah *server management platform*. ZenoPanel adalah *developer operations tool* — dibuat oleh developer, untuk developer yang ingin kontrol penuh tanpa overhead yang tidak perlu.

---

## 🤝 Kontribusi

Project ini dikembangkan secara pribadi dan terbuka untuk kontribusi. Jika menemukan bug atau ingin menambah fitur:

1. Fork repository ini
2. Buat branch baru (`git checkout -b feature/nama-fitur`)
3. Commit perubahan
4. Buat Pull Request

---

## 📄 Lisensi

Didistribusikan di bawah lisensi yang tersedia di file [LICENSE](./LICENSE).
