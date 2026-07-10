# ⚡ ZenoPanel

[![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![ZenoLang](https://img.shields.io/badge/engine-ZenoLang-purple?style=flat-square)](https://github.com/nextcore/zeno-rs)
[![License](https://img.shields.io/badge/license-Apache-blue?style=flat-square)](./LICENSE)
[![Version](https://img.shields.io/badge/version-v1.5.0-success?style=flat-square)](https://github.com/nextcore/zenopanel/releases/tag/v1.5.0)
[![RAM Usage](https://img.shields.io/badge/RAM-~15MB-brightgreen?style=flat-square)](#)
[![Single Binary](https://img.shields.io/badge/binary-single-red?style=flat-square)](#)
[![Alpine Linux](https://img.shields.io/badge/compatibility-Alpine_Linux-blue?logo=alpine-linux&style=flat-square)](#)

**ZenoPanel** adalah server management control panel generasi baru yang super cepat, sangat ringan (~15MB RAM), dan mandiri (*self-hosted*). Dibangun di atas engine proxy **Cloudflare Pingora** & **Zeno Rust** (runtime bahasa scripting *ZenoLang* berkinerja tinggi), ZenoPanel dirancang untuk para developer modern yang menginginkan kendali penuh atas server mereka — tanpa overhead, tanpa bloatware, tanpa kompromi.

ZenoPanel hadir sebagai **single binary** dengan zero external dependency: gateway reverse proxy Pingora, container runtime **Zeno Box** (OCI-compliant), database hosting, cloud backup, firewall, dan WAF — semua terintegrasi dalam satu binary yang berjalan native di **semua distribusi Linux**, termasuk Alpine Linux (MUSL/OpenRC).

---

## 🚀 Filosofi: Developer-First

ZenoPanel bukan sekadar panel hosting. Ini adalah **platform infrastruktur lengkap** yang dirancang dari awal untuk pengembang aplikasi modern — Rust, Go, Node.js, Python, .NET — yang membutuhkan efisiensi, otomasi, dan fleksibilitas tinggi dalam satu atap.

- **Zero Dependency**: Single static binary. Tidak ada runtime eksternal, tidak ada daemon tambahan, tidak ada package manager yang harus dijalankan sebelum panel bisa hidup.
- **Zero Bloatware**: Hanya mengonsumsi **~15 MB RAM** saat idle. Seluruh sumber daya server dapat dialokasikan penuh untuk aplikasi bisnis Anda.
- **Zero Downtime**: Semua perubahan konfigurasi — domain, SSL, proxy rules — diterapkan secara instan di memori tanpa restart gateway. Koneksi client aktif tidak pernah terputus.
- **Scriptable**: Seluruh logika panel ditulis dalam **ZenoLang**, bahasa scripting yang berjalan di atas Zeno Rust runtime. Anda dapat mengubah atau memperluas perilaku panel tanpa perlu mengkompilasi ulang binary Rust.

---

## ✨ Fitur-Fitur Unggulan

### 🖥️ Process Manager (Supervisord-Like)
- Kelola proses background aplikasi Anda (Node, Go, Python, dll) langsung dari UI web.
- **Auto-Restart Cerdas**: Pemulihan otomatis jika proses crash dengan algoritma *exponential backoff*.
- **Telemetry Real-Time**: Pantau beban CPU, RAM, dan status port aktif secara visual via Server-Sent Events (SSE).
- **Logs Streaming**: Streaming log stdout dan stderr secara asinkron dan real-time langsung ke browser via SSE.

### 🐳 Container Manager (Zeno Box OCI Engine)
- **Container Runtime Bawaan (Zeno Box)**: Jalankan container tanpa Docker daemon — menggunakan `runc` (OCI-compliant) yang di-embedded langsung di binary.
- **Pull Image dari Registry**: Dukung Docker Hub & OCI registry — pull image langsung via Registry API V2.
- **Manajemen Lengkap**: Create, start, stop, delete container — semuanya dari UI panel.
- **Volume Mount & Port Mapping**: Bind mount folder host, mapping port container.
- **Environment Variables**: Dukung env vars saat create container.
- **Browse Files Container**: Navigasi filesystem container langsung dari File Manager.
- **Real-Time Status**: Status container update otomatis secara instan via Server-Sent Events (SSE).
- **Rootless Mode**: Container bisa jalan tanpa hak root (menggunakan user namespace).
- **Native Checksum Fix**: Penanganan manual offloading TCP Checksum via system call `ioctl` di Rust — loopback NAT selalu lancar tanpa dependensi `ethtool`.

### 📦 Docker Compose Support
- **YAML Parser Bawaan**: Parse `docker-compose.yml` langsung — tanpa dependency eksternal.
- **Service Discovery**: Container bisa saling panggil via nama service (inject `/etc/hosts`).
- **Depends On**: Startup order sesuai dependency.
- **Networks**: Dukung definisi network dengan service discovery.
- **Relative Path Bind Mounts**: Mendukung pemetaan host path relatif (`./`, `../`) yang diterjemahkan relatif terhadap lokasi berkas `docker-compose.yml`.
- **Command Lengkap**: `compose up`, `compose down`, `compose ps` dari CLI & UI.
- **Boilerplate & Petunjuk Deployment**: Templat boilerplate Compose teroptimalisasi (Node.js, PHP Laravel FrankenPHP, Python FastAPI, Go, Java, dll.) langsung dari dasbor.

### 🔀 Reverse Proxy & Load Balancing Modern (Cloudflare Pingora)
- **Engine Pingora Terintegrasi**: Cloudflare Pingora Core — ultra-cepat, hemat memori, tahan buffer overflow.
- **Least Connections Load Balancing**: Distribusi trafik cerdas ke backend paling sedikit memegang koneksi aktif.
- **Active Health Checks & Process Awareness**: Monitoring kesehatan backend berkala. Jika aplikasi berhenti, Pingora otomatis mengembalikan halaman error 503 kustom.
- **Strip Path Prefix**: Potong prefix path secara dinamis sebelum meneruskan ke backend.
- **Dynamic Port Listeners**: Dukung rule proxy pada port non-standar.

### 🗄️ Database Manager
Deploy dan kelola database server langsung dari panel — terisolasi penuh di dalam kontainer **Zeno Box** (container OCI bawaan), tanpa instalasi manual atau polusi dependensi sistem.

- **Support MySQL 5.7, MySQL 8, dan PostgreSQL** — pilih versi sesuai kebutuhan.
- **Isolasi Penuh per Kontainer Zeno Box**: Setiap server database berjalan di dalam kontainer terisolasi tersendiri, tanpa konflik versi.
- **Connection Pooling Sidecar**: Dukungan connection pooling terintegrasi menggunakan sidecar container **ProxySQL** (untuk MySQL/MariaDB) dan **PgBouncer** (untuk PostgreSQL) untuk mengoptimalkan penggunaan resource koneksi database dan meningkatkan konkurensi.
- **Auto Health Check & Reconnect**: Pool koneksi MySQL/PostgreSQL di-ping otomatis setiap 60 detik. Jika koneksi terputus, reconnect dilakukan secara otomatis tanpa intervensi manual.
- **Manajemen Database & User**: Buat database, buat user, atur GRANT, dan ganti password — dari UI.
- **SQL Console Bawaan**: Eksekusi query SQL langsung dari browser.
- **Bulk Data Support**: Pool koneksi Rust internal mendukung operasi batch (500+ INSERT rows) dengan latensi sub-detik.
- **Visual Config Tuner**: Sesuaikan parameter performa database (max connections, buffer pool, max allowed packet) via antarmuka visual — tanpa edit `.cnf` atau `postgresql.conf` secara manual. Restart container otomatis setelah perubahan disimpan.
- **Database Maintenance**: `ANALYZE`, `OPTIMIZE`, `REPAIR TABLE` (MySQL) dan `ANALYZE`, `VACUUM` (PostgreSQL) langsung dari UI.

### 💾 Backup & Pemulihan (Otomatis + Cloud)
- **Auto Backup Terjadwal (Cron)**: Pencadangan database otomatis (per jam, harian, mingguan) dengan retensi yang dapat diatur.
- **Manual Trigger Backup**: Picu backup kapan saja langsung dari UI untuk database maupun volume kontainer.
- **Backup ke Cloud (S3-Compatible & Google Drive)**:
  - **S3-Compatible Storage**: Upload otomatis ke Cloudflare R2, MinIO, AWS S3, atau provider S3-compatible lainnya menggunakan **AWS Signature V4** yang diimplementasikan native di Rust.
  - **Google Drive via Service Account**: Upload ke Google Drive menggunakan autentikasi **JWT RSA-256** Google Service Account — tanpa binary eksternal seperti `rclone`.
- **Opsi Simpan Lokal**: Atur apakah file backup dipertahankan di disk server atau dihapus setelah diunggah ke cloud.
- **Backup Volume Kontainer**: Cadangkan folder data volume sebagai berkas `.tar.gz` terkompresi.
- **Kebijakan Retensi**: Hapus backup lokal lama secara otomatis berdasarkan jumlah yang dipertahankan.

### 🛡️ Web Application Firewall (WAF) & Rate Limiter

WAF ZenoPanel beroperasi di dua lapisan sekaligus: **Axum middleware** (panel) dan **Pingora gateway** (proxy traffic), memberikan perlindungan end-to-end tanpa konfigurasi tambahan.

- **Deteksi Ancaman Multi-Layer**:
  - SQL Injection (UNION-based, boolean-based, time-based blind, stacked queries)
  - Cross-Site Scripting / XSS (termasuk `data:`, `vbscript:`, SVG-based XSS)
  - Remote Code Execution / RCE (shell injection, PHP code injection, template injection)
  - Path Traversal (plain, URL-encoded, double-encoded, null byte)
  - **Server-Side Request Forgery / SSRF** — deteksi akses ke metadata cloud & network internal
  - **Log4Shell / JNDI Injection** — termasuk variant yang di-obfuscate
  - **Scanner & Attack Tool** — blokir otomatis User-Agent dari sqlmap, nikto, nmap, nuclei, acunetix, burpsuite, dll.
- **IP Block & Whitelist**: Block atau izinkan IP tertentu secara manual dari panel. Aturan bersifat **persistent** di database dan aktif instan tanpa restart.
- **Rate Limiting Granular**: Batasi request per IP dalam jendela waktu — konfigurasi dari UI tanpa restart.
- **Brute-Force Auto-Block**: IP yang gagal login sebanyak 5 kali secara otomatis diblokir dan masuk ke WAF blocklist secara permanen.
- **Security Response Headers**: Setiap response menyertakan `X-Frame-Options`, `X-Content-Type-Options`, `X-XSS-Protection`, dan `Referrer-Policy`.
- **Audit Log Real-Time**: Setiap serangan dicatat lengkap dengan IP, metode HTTP, kategori ancaman, severity, dan timestamp — tersedia di Security Tab.

### 🧱 Firewall Rules Manager (iptables)
- Kelola aturan `iptables` secara visual dari UI panel.
- **Persistent Rules**: Aturan firewall disimpan di database dan disinkronisasi ulang secara otomatis saat startup — tidak hilang setelah restart server.
- **Lockout Protection**: Cegah pemblokiran port SSH (22) dan port panel admin secara tidak sengaja.
- **Lockdown Mode**: Aktifkan kebijakan *default-DROP* instan — blokir semua, izinkan hanya port vital secara dinamis.

### 🔒 SSL/TLS Otomatis & HTTP/2 ALPN Native
- **HTTP/2 Multiplexing & ALPN** (`h2` dan `http/1.1`) native di handler TLS Pingora.
- **ACME Let's Encrypt**: Integrasi `instant-acme` dengan CSR berbasis `rcgen`.
- **Auto-Renewal**: Pembaruan sertifikat otomatis saat tersisa kurang dari 30 hari — zero-downtime certificate hot reload.

### 👥 Multi-User & Role-Based Access Control (RBAC)
- Tiga tingkatan role: **Admin**, **Editor**, dan **Viewer**.
- Autentikasi JWT via cookie HttpOnly.
- Perlindungan **CSRF** bawaan pada semua request modifikasi data.

### 🗃️ File Manager, Database Console, & Web Terminal
- **File Manager**: Navigasi direktori, unggah file via multipart, buat, edit, dan hapus berkas langsung dari browser.
- **Interactive Terminal**: Akses shell server secara aman di browser (khusus Administrator).

### 🔄 Self-Update Satu-Klik
- Deteksi rilis terbaru ZenoPanel secara real-time dari menu Pengaturan.
- **Hot Replacement**: Unlink binary lama sebelum mengunduh rilis baru — mencegah error *Text file busy* dan restart layanan secara aman.

### 🌐 Service Injector (Alpine Linux & OpenRC)
ZenoPanel mendeteksi lingkungan sistem init secara dinamis. Di Alpine Linux, panel secara otomatis:
- Menghasilkan skrip layanan OpenRC (`openrc-run`) native di `/etc/init.d/zenopanel`.
- Mendaftarkan startup otomatis via `rc-update add zenopanel default`.
- Menginisialisasi direktori data `/var/lib/zeno-container` tanpa campur tangan manual.

---

## 🏗️ Teknologi & Arsitektur

| Komponen | Teknologi |
| :--- | :--- |
| **Proxy Engine** | [Cloudflare Pingora](https://github.com/cloudflare/pingora) (`pingora-core` & `pingora-proxy`) |
| **Web Engine** | [Axum](https://github.com/tokio-rs/axum) & [Tokio](https://tokio.rs/) Async Runtime |
| **Container Runtime** | **Zeno Box** (berbasis [runc](https://github.com/opencontainers/runc) embedded) |
| **TLS & Crypto** | OpenSSL (Pingora handshake) & [Rustls](https://github.com/rustls/rustls) |
| **ACME & SSL** | [instant-acme](https://github.com/jsha/instant-acme), [rcgen](https://github.com/rustls/rcgen), [x509-parser](https://github.com/rusticata/x509-parser) |
| **Scripting Engine** | ZenoLang (custom scripting runtime di atas Zeno Rust) |
| **Cloud Backup** | AWS Signature V4 (S3) & Google JWT RSA-256 (Drive) — native Rust, tanpa `rclone` |

---

## 🗺️ Roadmap

### ✅ Sudah Tersedia
- Container runtime berbasis `runc` (embedded), pull dari Docker Hub, manajemen penuh dari UI
- Docker Compose — YAML parser bawaan, service discovery, depends-on, networks
- Rootless container support
- TCP port proxy & Network bridge (veth pair + loopback NAT)
- Integrasi File Manager untuk filesystem container
- Volume & network management dinamis dari UI
- Resource limits (RAM & CPU) per kontainer
- Health checks & auto-restart kontainer
- Self-update satu-klik tanpa *Text file busy*
- Database hosting di kontainer terisolasi **Zeno Box** (MySQL 5.7/8, PostgreSQL) dengan Connection Pooling (ProxySQL & PgBouncer)
- Visual Config Tuner database
- Database maintenance (ANALYZE / OPTIMIZE / REPAIR / VACUUM) dari UI
- Auto & manual backup database + volume ke cloud (S3-compatible & Google Drive)
- **WAF** multi-layer (SQLi, XSS, RCE, Path Traversal, SSRF, Log4Shell, Scanner Bot)
- **IP Block/Whitelist** — persistent di DB, live update tanpa restart
- **Brute-force auto-block** — IP diblokir permanen setelah 5x gagal login
- **Security response headers** bawaan
- **Firewall rules persistent** — bertahan setelah restart, sinkronisasi otomatis saat startup
- **DB health check** — reconnect otomatis jika pool MySQL/PostgreSQL terputus
- Rate Limiter, SSL/TLS otomatis ACME Let's Encrypt & auto-renewal
- Multi-User RBAC (Admin / Editor / Viewer)
- Service Injector untuk Alpine Linux OpenRC

### 🚧 Sedang Dikembangkan
- Container Build dari Dockerfile
- Dukungan Container Registry privat (login & pull dari registry privat)
- Remote Database Access (akses ZenoBox dari luar server secara aman)

---

## 📥 Instalasi (Production)

```bash
curl -fsSL https://raw.githubusercontent.com/nextcore/zenopanel/main/install.sh | bash
```

*ZenoPanel dipasang di `/opt/zenopanel` secara default. Direktori dapat disesuaikan secara interaktif selama script berjalan.*

Untuk panduan instalasi manual dan kustomisasi lokasi data, lihat [install.md](./install.md).

---

## 🛠️ Pengembangan Lokal

### Prasyarat
Pastikan kompiler Rust (stable) sudah terpasang.

### Build
```bash
git clone https://github.com/nextcore/zenopanel.git
cd zenopanel
cargo build --release
```

### Jalankan
```bash
cp .env.example .env
PATH=$PWD/cmake_local/bin:$PATH cargo run
```

Buka `http://localhost:3001/zpanel` di browser. Untuk detail port dan konfigurasi lokal, lihat [development.md](./development.md). Untuk kompilasi static MUSL (Alpine) atau GLIBC 2.17, lihat [compile.md](./compile.md).

---

## 🤝 Kontribusi & Lisensi

ZenoPanel didistribusikan di bawah lisensi [Apache 2.0](./LICENSE). Kami menyambut kontribusi kode, pelaporan bug, dan saran fitur melalui Pull Request dan Issues di GitHub.
