# ⚡ ZenoPanel

[![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![ZenoCore](https://img.shields.io/badge/engine-ZenoCore_v0.2.0-purple?style=flat-square)](https://crates.io/crates/zenocore)
[![License](https://img.shields.io/badge/license-Apache-blue?style=flat-square)](./LICENSE)
[![Version](https://img.shields.io/badge/version-v1.7.0-success?style=flat-square)](https://github.com/nextcore/zenopanel/releases/tag/v1.7.0)
[![RAM Usage](https://img.shields.io/badge/RAM-~15MB-brightgreen?style=flat-square)](#)
[![Single Binary](https://img.shields.io/badge/binary-single-red?style=flat-square)](#)
[![Alpine Linux](https://img.shields.io/badge/compatibility-Alpine_Linux-blue?logo=alpine-linux&style=flat-square)](#)

**ZenoPanel** adalah server management control panel generasi baru yang super cepat, sangat ringan (~15MB RAM), dan mandiri (*self-hosted*). Dibangun di atas engine proxy **Cloudflare Pingora** & **ZenoCore Rust** (runtime bahasa scripting *ZenoLang* berkinerja tinggi), ZenoPanel dirancang untuk para developer modern yang menginginkan kendali penuh atas server mereka — tanpa overhead, tanpa bloatware, tanpa kompromi.

ZenoPanel hadir sebagai **single binary** dengan zero external dependency: gateway reverse proxy Pingora, container runtime **Zeno Box** (OCI-compliant), MicroVM engine **Zeno Machine** (Cloud-Hypervisor), database hosting, cloud backup, firewall, dan WAF — semua terintegrasi dalam satu binary yang berjalan native di **semua distribusi Linux**, termasuk Alpine Linux (MUSL/OpenRC).

---

## 🚀 Filosofi: Developer-First

ZenoPanel bukan sekadar panel hosting. Ini adalah **platform infrastruktur lengkap** yang dirancang dari awal untuk pengembang aplikasi modern — Rust, Go, Node.js, Python, .NET — yang membutuhkan efisiensi, otomasi, dan fleksibilitas tinggi dalam satu atap.

- **Zero Dependency**: Single static binary. Tidak ada runtime eksternal, tidak ada daemon tambahan, tidak ada package manager yang harus dijalankan sebelum panel bisa hidup.
- **Zero Bloatware**: Hanya mengonsumsi **~15 MB RAM** saat idle. Seluruh sumber daya server dapat dialokasikan penuh untuk aplikasi bisnis Anda.
- **Zero Downtime**: Semua perubahan konfigurasi — domain, SSL, proxy rules — diterapkan secara instan di memori tanpa restart gateway. Koneksi client aktif tidak pernah terputus.
- **Scriptable**: Seluruh logika panel ditulis dalam **ZenoLang**, bahasa scripting yang berjalan di atas ZenoCore v0.2.0 Rust runtime. Anda dapat mengubah atau memperluas perilaku panel tanpa perlu mengkompilasi ulang binary Rust.

---

## ✨ Fitur-Fitur Unggulan

### ⚙️ ZenoCore v0.2.0 & Dynamic Plugin Engine (NEW in v1.7.0)
ZenoPanel kini didukung penuh oleh **`zenocore v0.2.0`** resmi dari [crates.io](https://crates.io/crates/zenocore):
- **50+ Slot Bawaan (Standard Library)**: Dukungan manipulasi string (`string.*`), kalkulasi matematika (`math.*`), operasi array & map (`array.*`/`map.*`), evaluasi `if` kompleks (`&&`/`||`), dan casting tipe data.
- **Native Dynamic Plugin System (`plugin.load`)**: Muat extension plugin `.so`/`.dylib` yang dikompilasi dari Rust secara *runtime* tanpa perlu mengompilasi ulang binary ZenoPanel utama.
- **Thread-Safe Architecture**: Arsitektur interior mutability `Mutex` yang *thread-safe* untuk eksekusi concurrent pada multi-threaded Axum/Tokio web server.

### 🖥️ Zeno Machine — MicroVM Engine
Engine virtualisasi MicroVM terbaru berbasis **[Cloud-Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor)** — hypervisor generasi baru yang ditulis dalam Rust, ringan, dan aman.

- **Buat & Kelola MicroVM**: Jalankan VM Linux atau Windows dengan konfigurasi vCPU & RAM yang dapat disesuaikan — langsung dari UI panel.
- **Live Hot-Plug (vCPU & RAM)**: Ubah alokasi resource VM secara *real-time* tanpa reboot.
- **Live Migration (KVM Memory Sync)**: Pindahkan VM yang sedang aktif antar server fisik dengan jeda handoff kurang dari 50ms — tanpa memutuskan koneksi pengguna.
- **Dashboard Statistik**: Ringkasan total VM, VM aktif, total vCPU dan RAM terpakai ditampilkan secara live.
- **Binary Cloud-Hypervisor Terintegrasi**: Binary `cloud-hypervisor-static` (musl/Alpine) disertakan langsung di dalam paket distribusi.

### 🔄 Live Migration Dual-Engine
Live Migration tersedia untuk dua engine sekaligus dengan **Manual Approval Handshake** — persetujuan manual diperlukan dari Administrator server tujuan sebelum transmisi dimulai.

- **Zeno Machine Migration (KVM)**: Transmisi memori VM secara pre-copy dengan dirty page sync. Jeda freeze <50ms.
- **Zeno Box Migration (CRIU)**: Checkpoint/Restore proses container dengan socket TCP aktif dipertahankan. Waktu transmisi <200ms.
- **Pre-flight Compatibility Check**: Verifikasi otomatis konektivitas, latensi, ketersediaan RAM, dan kompatibilitas kernel sebelum migrasi dapat dimulai.
- **Dual-Host Progress Tracking**: Progress bar HOST A (sumber) dan HOST B (tujuan) ditampilkan secara bersamaan, dengan label fase teknis yang update tiap langkah.
- **Rollback Otomatis**: VM/Container tetap berjalan di HOST A jika proses migrasi ke HOST B gagal.
- **Glowing Approval Banner**: Notifikasi bercahaya muncul di dashboard server penerima dengan tombol Accept & Reject.

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
- **Live Migration (CRIU)**: Pindahkan container yang aktif ke server lain tanpa menghentikan proses.

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
- **Connection Pooling Sidecar**: Dukungan connection pooling terintegrasi menggunakan sidecar container **ProxySQL** (untuk MySQL/MariaDB) dan **PgBouncer** (untuk PostgreSQL).
- **Auto Health Check & Reconnect**: Pool koneksi MySQL/PostgreSQL di-ping otomatis setiap 60 detik.
- **Manajemen Database & User**: Buat database, buat user, atur GRANT, dan ganti password — dari UI.
- **SQL Console Bawaan**: Eksekusi query SQL langsung dari browser.
- **Bulk Data Support**: Pool koneksi Rust internal mendukung operasi batch (500+ INSERT rows) dengan latensi sub-detik.
- **Visual Config Tuner**: Sesuaikan parameter performa database via antarmuka visual — tanpa edit `.cnf` secara manual.
- **Database Maintenance**: `ANALYZE`, `OPTIMIZE`, `REPAIR TABLE` (MySQL) dan `ANALYZE`, `VACUUM` (PostgreSQL) langsung dari UI.

### 💾 Backup & Pemulihan (Otomatis + Cloud)
- **Auto Backup Terjadwal (Cron)**: Pencadangan database otomatis (per jam, harian, mingguan) dengan retensi yang dapat diatur.
- **Manual Trigger Backup**: Picu backup kapan saja langsung dari UI.
- **Backup ke Cloud (S3-Compatible & Google Drive)**:
  - **S3-Compatible Storage**: Upload otomatis ke Cloudflare R2, MinIO, AWS S3, menggunakan **AWS Signature V4** native Rust.
  - **Google Drive via Service Account**: Upload ke Google Drive menggunakan **JWT RSA-256** tanpa binary eksternal seperti `rclone`.
- **Backup Volume Kontainer**: Cadangkan folder data volume sebagai berkas `.tar.gz`.
- **Kebijakan Retensi**: Hapus backup lokal lama secara otomatis.

### 🛡️ Web Application Firewall (WAF) & Rate Limiter
WAF beroperasi di dua lapisan: **Axum middleware** (panel) dan **Pingora gateway** (proxy traffic).

- **Deteksi Ancaman Multi-Layer**: SQL Injection, XSS, RCE, Path Traversal, SSRF, Log4Shell/JNDI, Scanner/Attack Tool.
- **Dynamic Bot Detection & Whitelisting**: Izinkan Googlebot, Bingbot, Yandex, dan tambahkan regex kustom.
- **Per-Website IP Access Control**: Whitelist/blacklist IP per website (mendukung CIDR).
- **Mode Log-Only (Dry Run) Default**: Minimalisasi false positive.
- **IP Block & Whitelist**: Persistent di database, aktif instan.
- **Rate Limiting Granular**: Batasi request per IP dalam jendela waktu — konfigurasi dari UI.
- **Brute-Force Auto-Block**: IP yang gagal login 5 kali otomatis diblokir permanen.
- **Audit Log Real-Time**: Setiap serangan dicatat lengkap dengan IP, kategori ancaman, dan timestamp.

### 🧱 Firewall Rules Manager (iptables)
- Kelola aturan `iptables` secara visual dari UI panel.
- **Persistent Rules**: Disimpan di database, disinkronisasi ulang saat startup.
- **Lockout Protection**: Cegah pemblokiran port SSH dan port panel admin secara tidak sengaja.
- **Lockdown Mode**: Aktifkan kebijakan *default-DROP* instan.

### 🔒 SSL/TLS Otomatis & HTTP/2 ALPN Native
- **HTTP/2 Multiplexing & ALPN** (`h2` dan `http/1.1`) native di handler TLS Pingora.
- **ACME Let's Encrypt**: Integrasi `instant-acme` dengan CSR berbasis `rcgen`.
- **Auto-Renewal**: Pembaruan sertifikat otomatis saat tersisa kurang dari 30 hari — zero-downtime certificate hot reload.

### 👥 Multi-User & Role-Based Access Control (RBAC)
- Tiga tingkatan role: **Admin**, **Editor**, dan **Viewer**.
- Autentikasi JWT via cookie HttpOnly.
- Perlindungan **CSRF** bawaan pada semua request modifikasi data.

### 🗃️ File Manager, Database Console, & Web Terminal
- **File Manager**: Navigasi direktori, unggah file via multipart, buat, edit, dan hapus berkas. Mendukung operasi massal (bulk copy, cut, paste).
- **Interactive Terminal**: Akses shell server secara aman di browser (khusus Administrator).

### 🔄 Self-Update Satu-Klik
- Deteksi rilis terbaru ZenoPanel secara real-time dari menu Pengaturan.
- **Hot Replacement**: Unlink binary lama sebelum mengunduh rilis baru — mencegah error *Text file busy*.

### 🌐 Service Injector (Alpine Linux & OpenRC)
ZenoPanel mendeteksi lingkungan sistem init secara dinamis. Di Alpine Linux, panel secara otomatis:
- Menghasilkan skrip layanan OpenRC native di `/etc/init.d/zenopanel`.
- Mendaftarkan startup otomatis via `rc-update add zenopanel default`.
- Menginisialisasi direktori data `/var/lib/zeno-container` tanpa campur tangan manual.

---

## 🏗️ Teknologi & Arsitektur

| Komponen | Teknologi |
| :--- | :--- |
| **Proxy Engine** | [Cloudflare Pingora](https://github.com/cloudflare/pingora) (`pingora-core` & `pingora-proxy`) |
| **Web Engine** | [Axum](https://github.com/tokio-rs/axum) & [Tokio](https://tokio.rs/) Async Runtime |
| **Scripting Engine** | **ZenoLang** powered by [`zenocore v0.2.0`](https://crates.io/crates/zenocore) & [`zenoengine v0.2.0`](https://crates.io/crates/zenoengine) |
| **Container Runtime** | **Zeno Box** (berbasis [runc](https://github.com/opencontainers/runc) embedded) |
| **MicroVM Engine** | **Zeno Machine** (berbasis [Cloud-Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) static/musl) |
| **Live Migration** | KVM Memory Sync (Zeno Machine) & [CRIU](https://criu.org/) Checkpoint/Restore (Zeno Box) |
| **TLS & Crypto** | OpenSSL (Pingora handshake) & [Rustls](https://github.com/rustls/rustls) |
| **ACME & SSL** | [instant-acme](https://github.com/jsha/instant-acme), [rcgen](https://github.com/rustls/rcgen), [x509-parser](https://github.com/rusticata/x509-parser) |
| **Cloud Backup** | AWS Signature V4 (S3) & Google JWT RSA-256 (Drive) — native Rust, tanpa `rclone` |

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

---

## 🤝 Kontribusi & Lisensi

ZenoPanel didistribusikan di bawah lisensi [Apache 2.0](./LICENSE). Kami menyambut kontribusi kode, pelaporan bug, dan saran fitur melalui Pull Request dan Issues di GitHub.
