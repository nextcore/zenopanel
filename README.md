# ⚡ ZenoPanel

[![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![ZenoLang](https://img.shields.io/badge/engine-ZenoLang-purple?style=flat-square)](https://github.com/nextcore/zeno-rs)
[![License](https://img.shields.io/badge/license-Apache-blue?style=flat-square)](./LICENSE)
[![RAM Usage](https://img.shields.io/badge/RAM-~15MB-brightgreen?style=flat-square)](#)
[![Single Binary](https://img.shields.io/badge/binary-single-red?style=flat-square)](#)

**ZenoPanel** adalah server management control panel generasi baru yang super cepat, sangat ringan (~15MB RAM), dan mandiri (*self-hosted*). Dibangun di atas **Zeno Rust** (runtime bahasa scripting *ZenoLang* berkinerja tinggi), ZenoPanel dirancang khusus untuk para developer modern yang menginginkan kendali server penuh tanpa overhead dan kompleksitas panel tradisional.

Tidak seperti aaPanel atau 1Panel yang menginstal ratusan megabyte dependensi pihak ketiga, ZenoPanel hadir sebagai **single binary** dengan persistensi database SQLite lokal. **Zero dependency, zero bloatware.**

---

## 🚀 Kenapa ZenoPanel Berbeda? (Developer-First Philosophy)

aaPanel dan 1Panel dirancang untuk sysadmin tradisional yang mengelola web hosting LAMP/LEMP klasik. ZenoPanel dirancang untuk pengembang aplikasi modern (Rust, Go, Node.js, Python, .NET) yang membutuhkan kecepatan, efisiensi tinggi, dan integrasi mulus.

| Dimensi | aaPanel / 1Panel | ⚡ ZenoPanel |
| :--- | :--- | :--- |
| **Konsumsi RAM (Idle)** | 1.2 GB - 2 GB RAM | **~15 MB RAM** (Hemat 99%) |
| **Instalasi & Setup** | Butuh bash script kompleks, mengunduh Nginx, PHP, MySQL, dll. | **Langsung Jalankan** (Single binary + SQLite). |
| **Deployment App** | Harus membuat config vhost manual, unit systemd manual. | **Satu Klik Form**: Masukkan command, env, cwd. Otomatis jalan dengan monitoring. |
| **Isolasi Lingkungan** | Variabel lingkungan global bercampur dengan proses sistem. | **Isolasi Penuh**: Env vars dienkapsulasi aman per proses. |
| **Reverse Proxy** | Edit file konfigurasi Nginx, reload manual, resiko syntax error. | **Dynamic Rules**: Konfigurasi instan via UI tanpa menyentuh file config. |
| **Kustomisasi Panel** | Harus membongkar ribuan baris PHP/Go dan compile ulang. | **ZenoLang Scripting**: Ubah logika panel secara dinamis tanpa compile ulang Rust. |

---

## ✨ Fitur-Fitur Unggulan

### 🖥️ Process Manager (Supervisord-Like)
- Kelola proses background aplikasi Anda (Node, Go, Python, dll) langsung dari UI web.
- **Auto-Restart Cerdas**: Pemulihan otomatis jika proses crash dengan algoritma *exponential backoff*.
- **Telemetry Real-Time**: Pantau beban CPU, RAM, dan status port aktif secara visual.
- **Logs Streaming**: Streaming log stdout dan stderr secara real-time langsung ke browser Anda.

### 🔀 Reverse Proxy & Load Balancing Modern
- **Least Connections Load Balancing**: Pembagian trafik cerdas ke target backend yang paling sedikit memegang koneksi aktif.
- **Active Health Checks**: Worker background yang memantau kesehatan target secara berkala dan memutus rute ke target yang mati secara otomatis.
- **Strip Path Prefix**: Memotong prefix path sebelum meneruskannya ke backend.
- **Dynamic Port Listeners**: Mendukung rule proxy untuk mendengarkan port non-standar di server.

### 🛡️ Web Application Firewall (WAF) & Rate Limiter
- **Keamanan WAF Bawaan**: Deteksi dan cegah serangan SQL Injection, XSS, Path Traversal, dan Remote Code Execution (RCE).
- **Rate Limiting Granular**: Batasi request maksimum per IP dalam jendela waktu tertentu untuk menangkis serangan DDoS dan abuse API.
- **Dedicated Security Tab (Khusus Admin)**: Halaman khusus untuk menyetel konfigurasi WAF/Rate Limiting serta memantau log audit trail serangan secara real-time.

### 🔒 SSL/TLS Otomatis & HTTP/2 ALPN Native
- **Protokol Cepat**: Dukungan HTTP/2 Multiplexing & ALPN (`h2` dan `http/1.1`) secara bawaan tanpa Nginx.
- **ACME Let's Encrypt Asli**: Integrasi pustaka produksi `instant-acme` dengan CSR berbasis standard `rcgen`.
- **Auto-Renewal Cerdas**: Pemantauan sertifikat asli via parser X.509 (`x509-parser`) yang memperbarui sertifikat otomatis saat masa berlaku tersisa kurang dari 30 hari.

### 👥 Multi-User & Role-Based Access Control (RBAC)
- Tiga tingkatan role terverifikasi: **Admin**, **Editor**, dan **Viewer**.
- Autentikasi JWT yang aman menggunakan cookie HttpOnly.
- Perlindungan **CSRF** bawaan pada semua request modifikasi data (POST/PUT/DELETE).

### 🗃️ Database Console, File Manager, & Web Terminal
- **Database Console**: Jalankan query SQL langsung dari UI untuk database SQLite internal atau default aplikasi.
- **File Manager**: Navigasi direktori, unggah file via *multipart forms*, buat, edit, dan hapus berkas server dari peramban.
- **Interactive Terminal**: Akses shell server langsung secara aman di browser (khusus Administrator).

---

## 🏗️ Teknologi & Arsitektur

ZenoPanel dibangun di atas fondasi teknologi Rust yang kokoh untuk menjamin efisiensi dan keamanan maksimal:

- **Web Engine**: [Axum](https://github.com/tokio-rs/axum) & [Tokio](https://tokio.rs/) Async Runtime.
- **TLS Engine**: [Rustls](https://github.com/rustls/rustls) & [tokio-rustls](https://github.com/tokio-rs/tls).
- **Scripting Engine**: ZenoEngine (ZenoLang Runtime) & Zeno-Blade (Blade-style template engine).
- **Security & ACME**: [instant-acme](https://github.com/jsha/instant-acme), [rcgen](https://github.com/rustls/rcgen), & [x509-parser](https://github.com/rusticata/x509-parser).

---

## 🗺️ Roadmap Masa Depan (Zeno Container & Runtimes)

Kami sedang aktif mengembangkan dukungan orkestrasi kontainer langsung dari panel:

- **Zeno Container (Fase 1)**: Integrasi dengan Docker Daemon (`/var/run/docker.sock`) menggunakan `bollard` untuk manajemen kontainer standar industri dari UI.
- **Language Runtime Manager (Fase 2)**: Deployment aplikasi multi-bahasa satu-klik menggunakan container runtime (Node.js, Python, PHP, Go, .NET) dengan versi bahasa yang dapat dipilih oleh pengguna tanpa menulis Dockerfile secara manual.

---

## 🚀 Memulai ZenoPanel dalam 3 Langkah

### 1. Prasyarat
Pastikan server Anda memiliki runtime Rust stable terinstal.

### 2. Clone & Build
```bash
git clone https://github.com/nextcore/zenopanel.git
cd zenopanel
cargo build --release
```

### 3. Konfigurasi & Jalankan
Salin berkas konfigurasi default:
```bash
cp .env.example .env
```
Sesuaikan konfigurasi port dan kredensial admin di file `.env`, lalu jalankan panel:
```bash
cargo run --release
```

Server Anda kini aktif! Buka `http://localhost:3000/login` (atau sub-path login khusus yang Anda setel di `.env`) untuk masuk ke dashboard.

---

## 🤝 Kontribusi & Lisensi

ZenoPanel didistribusikan di bawah lisensi [Apache 2.0](./LICENSE). Kami sangat menyambut kontribusi kode, pelaporan bug, dan saran fitur melalui Pull Request dan Issues di GitHub.
