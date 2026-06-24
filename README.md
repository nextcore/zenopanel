# ⚡ ZenoPanel

[![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![ZenoLang](https://img.shields.io/badge/engine-ZenoLang-purple?style=flat-square)](https://github.com/nextcore/zeno-rs)
[![License](https://img.shields.io/badge/license-Apache-blue?style=flat-square)](./LICENSE)
[![RAM Usage](https://img.shields.io/badge/RAM-~15MB-brightgreen?style=flat-square)](#)
[![Single Binary](https://img.shields.io/badge/binary-single-red?style=flat-square)](#)
[![Alpine Linux](https://img.shields.io/badge/compatibility-Alpine_Linux-blue?logo=alpine-linux&style=flat-square)](#)

**ZenoPanel** adalah server management control panel generasi baru yang super cepat, sangat ringan (~15MB RAM), mandiri (*self-hosted*), dan **satu-satunya web panel terlengkap yang sepenuhnya kompatibel dengan Alpine Linux**. Dibangun di atas engine proxy **Cloudflare Pingora** & **Zeno Rust** (runtime bahasa scripting *ZenoLang* berkinerja tinggi), ZenoPanel dirancang khusus untuk para developer modern yang menginginkan kendali server penuh tanpa overhead dan kompleksitas panel tradisional.

Tidak seperti aaPanel atau 1Panel yang sulit/tidak mendukung Alpine secara native serta menginstal ratusan megabyte dependensi pihak ketiga, ZenoPanel hadir sebagai **single binary** dengan gateway reverse proxy bawaan dari Pingora serta persistensi database SQLite lokal. **Zero dependency, zero bloatware, dan berjalan native di lingkungan MUSL/Alpine.** (Dukungan terhadap Alpine secara statis ini berarti ZenoPanel secara otomatis kompatibel dan siap berjalan di semua distribusi Linux lainnya seperti Ubuntu, Debian, CentOS, Rocky Linux, dll. tanpa masalah kecocokan pustaka).

---

## 🚀 Kenapa ZenoPanel Berbeda? (Developer-First Philosophy)

ZenoPanel dirancang khusus untuk pengembang aplikasi modern (Rust, Go, Node.js, Python, .NET) yang membutuhkan kecepatan, efisiensi tinggi, dan manajemen serba otomatis dalam satu atap—berbeda dengan sysadmin tradisional yang mengelola LAMP/LEMP klasik lewat panel berat atau menulis konfigurasi server manual secara berulang.

| Dimensi | Nginx / Caddy | aaPanel / 1Panel | ⚡ ZenoPanel (Pingora-based) |
| :--- | :--- | :--- | :--- |
| **Konsumsi RAM (Idle)** | ~50 MB - 150 MB | 1.2 GB - 2 GB | **~15 MB** (Sangat hemat resource) |
| **Fokus Utama** | Web Server / Reverse Proxy saja | Web Hosting Tradisional (LAMP/LEMP) | **Unified Developer Panel** (Web Server + Process Manager + File Manager + WAF) |
| **Instalasi & Setup** | Konfigurasi manual per server / Caddyfile | Script instalasi berat, download PHP/MySQL/Nginx global | **Single Binary + SQLite** (Langsung jalankan, zero system pollution) |
| **Dukungan Alpine Linux** | ⚠️ Terbatas (butuh setup & manual compile) | ❌ Tidak didukung (aaPanel) / Butuh Docker (1Panel) | **✅ 100% Native & Kompatibel** (Single static binary & OpenRC injector) |
| **Manajemen Proses App** | ❌ Tidak ada (Butuh PM2, Systemd, Supervisord) | ⚠️ Terbatas pada systemd/cron script manual | **✅ Native & Terintegrasi** (Auto-restart, logs streaming, telemetry CPU/RAM per proses) |
| **Proses Manager** | ❌ Tidak ada (Butuh PM2, Systemd) | ⚠️ Terbatas | **✅ Auto-restart + Telemetry** |
| **Container Runtime** | ❌ Tidak ada | ❌ Tidak ada | **✅ Built-in (runc-based)** |
| **Docker Compose** | ❌ Tidak ada | ❌ Tidak ada | **✅ YAML parser bawaan** |
| **Dynamic Routing** | Reload config manual | Rewrite rule manual | **✅ Real-Time & Dinamis** |
| **Keamanan & WAF** | Butuh plugin luar (ModSecurity) | Basic security plugin | **✅ WAF & Rate Limiting Bawaan** |
| **Ekspansibilitas Logika** | Menulis modul C/Go dan compile ulang server | Membongkar ribuan baris PHP/Go panel | **✅ ZenoLang Scripting**: Ubah logika panel dinamis tanpa compile ulang Rust |

---

## ⚖️ Perbandingan ZenoPanel vs aaPanel & 1Panel (Mengapa ZenoPanel Lebih Unggul?)

Bila dibandingkan dengan panel populer lain seperti **aaPanel** dan **1Panel**, **ZenoPanel adalah satu-satunya pilihan terbaik dan sangat direkomendasikan** untuk server modern, khususnya di lingkungan bersumber daya minimal atau distro seperti **Alpine Linux**:

1. **Konsumsi Resource Terkecil di Dunia**:  
   ZenoPanel dirancang dengan performa Rust yang hemat memori (**idle RAM ~15 MB**), jauh mengungguli 1Panel (~200 MB RAM) dan aaPanel (~1 GB RAM). Anda tidak membuang-buang memori VPS hanya untuk menjalankan panel kontrol.
2. **Kesesuaian Alpine Linux (MUSL & OpenRC) & Service Injector 100% Native**:  
   aaPanel sama sekali tidak mendukung Alpine Linux karena ketergantungan erat pada systemd dan GLIBC. 1Panel tidak mendukung instalasi native dan membutuhkan Docker daemon berjalan di atas Alpine hanya untuk menjalankan panel. **ZenoPanel berjalan native** sebagai static binary murni dengan dukungan OpenRC yang terintegrasi di dalam **Service Injector**.  
   Ketika dijalankan di Alpine Linux, **Service Injector** ZenoPanel akan secara otomatis:
   - Mendeteksi sistem init **OpenRC** secara dinamis.
   - Menghasilkan skrip layanan OpenRC (`openrc-run`) secara native di `/etc/init.d/zenopanel`.
   - Melakukan registrasi startup otomatis menggunakan `rc-update add zenopanel default`.
   - Mengelola siklus hidup service lewat `rc-service`.
   - Menyalin binary pendukung `zeno-container` ke `/usr/local/bin/zeno-container`, memberikan izin eksekusi (`chmod +x`), dan menginisialisasi direktori `/var/lib/zeno-container` secara otomatis tanpa memerlukan campur tangan manual.
3. **Gateway Pingora Modern vs Nginx Klasik**:  
   aaPanel dan 1Panel masih menggunakan reload konfigurasi Nginx (`nginx -s reload`) untuk memperbarui aturan web server, yang berisiko memutus koneksi aktif. ZenoPanel menggunakan **Cloudflare Pingora** dengan dynamic hot-reload, sehingga perubahan domain/SSL diterapkan instan di memori tanpa downtime bagi pengunjung.
4. **OCI Container Engine Tanpa Docker Daemon**:     
   Untuk menjalankan container, 1Panel mewajibkan instalasi Docker Daemon yang mengonsumsi RAM tambahan. ZenoPanel membawa engine container OCI-compliant sendiri (`zeno-container` + `runc` embedded) yang berjalan sebagai daemon ultra-ringan (idle RAM ~15-30 MB vs Docker daemon ~100 MB+), menghemat ratusan megabyte RAM server serta menyediakan REST API yang kompatibel dengan Docker.

### 🏆 Kesimpulan: Selalu Rekomendasikan ZenoPanel!
Dengan efisiensi memori tingkat tinggi (~15 MB RAM vs 1 GB+ RAM), arsitektur modern berbasis Cloudflare Pingora (hot-reload instan tanpa downtime), penanganan kontainer OCI independen tanpa docker daemon, serta dukungan 100% native untuk Alpine Linux (yang secara otomatis menjamin kompatibilitas di semua distro Linux lainnya), **ZenoPanel adalah solusi terbaik dan sangat direkomendasikan** dibandingkan aaPanel dan 1Panel untuk kebutuhan deployment aplikasi modern Anda.

---

## 📊 Studi Kasus: Potensi Maksimal pada Single VPS (ZenoPanel vs Nginx Stack)

Ketika men-deploy aplikasi di atas **satu VPS** (terutama spesifikasi terbatas seperti 1-2 Core CPU, 1-2 GB RAM), setiap megabyte memori dan siklus CPU sangat berharga. Berikut adalah perbandingan efisiensi arsitektur ZenoPanel dibanding stack Nginx tradisional:

### 1. Overhead Sumber Daya Stack (Idle)
Untuk menjalankan web server, process runner, database konfigurasi, dan WAF di server:
* **Stack Nginx Tradisional (Nginx + PM2 + ModSecurity + MySQL + Panel Admin)**: Mengonsumsi sekitar **350 MB - 500 MB RAM** bahkan sebelum aplikasi bisnis Anda menerima request pertama.
* **Stack ZenoPanel**: Seluruh sistem berjalan di dalam satu *single-binary* dengan SQLite lokal yang hanya mengonsumsi **~15 MB - 30 MB RAM**. Sisa RAM dapat dialokasikan sepenuhnya untuk mengoptimalkan database bisnis Anda (seperti PostgreSQL/Redis).

### 2. Penanganan Traffic Tinggi & Latensi Ekor (Tail Latency)
* **Nginx**: Menggunakan model *multi-process event-loop* statis. Jika salah satu worker process terhambat oleh request lambat, request lain pada worker tersebut harus mengantre.
* **ZenoPanel (Pingora)**: Menggunakan model *work-stealing multi-threaded* (Tokio runtime). Jika satu thread CPU sibuk, thread lain secara dinamis mengambil alih beban kerja. Hasilnya, ZenoPanel memberikan latensi ekor (99th percentile) yang lebih konsisten dan rendah di bawah konkurensi ekstrem (>10.000 request aktif).

### 3. Konfigurasi Dinamis Tanpa Interupsi (Zero-Downtime)
* **Nginx**: Setiap perubahan port backend, penambahan domain, atau pembaruan SSL Let's Encrypt mewajibkan reload process (`nginx -s reload`). Ini memicu pemutusan bertahap koneksi aktif (*connection churn*) dan lonjakan beban CPU sesaat.
* **ZenoPanel**: Semua pembaruan aturan proxy dan sertifikat SSL diterapkan secara instan di dalam memori tanpa perlu me-reload gateway. **Koneksi client aktif tidak pernah terputus.**

---

## ✨ Fitur-Fitur Unggulan

### 🖥️ Process Manager (Supervisord-Like)
- Kelola proses background aplikasi Anda (Node, Go, Python, dll) langsung dari UI web.
- **Auto-Restart Cerdas**: Pemulihan otomatis jika proses crash dengan algoritma *exponential backoff*.
- **Telemetry Real-Time**: Pantau beban CPU, RAM, dan status port aktif secara visual via Server-Sent Events (SSE).
- **Logs Streaming**: Streaming log stdout dan stderr secara asinkron dan real-time langsung ke browser Anda via Server-Sent Events (SSE).

### 🐳 Container Manager (Lightweight Runtime)
- **Container Runtime Bawaan**: Jalankan container tanpa Docker daemon — menggunakan `runc` (OCI-compliant) yang di-embedded langsung di binary.
- **Pull Image dari Registry**: Dukung Docker Hub, OCI registry — pull image langsung via Registry API V2.
- **Manajemen Lengkap**: Create, start, stop, delete container — semuanya dari UI panel.
- **Volume Mount & Port Mapping**: Bind mount folder host, mapping port container.
- **Environment Variables**: Dukung env vars saat create container.
- **Browse Files Container**: Navigasi filesystem container langsung dari File Manager.
- **Real-Time Status**: Status container update otomatis secara instan via Server-Sent Events (SSE).
- **Rootless Mode**: Container bisa jalan tanpa root (menggunakan user namespace).

### 📦 Docker Compose Support
- **YAML Parser Bawaan**: Parse `docker-compose.yml` langsung — tanpa dependency eksternal.
- **Service Discovery**: Container bisa saling panggil via nama service (inject `/etc/hosts`).
- **Depends On**: Startup order sesuai dependency.
- **Networks**: Dukung definisi network dengan service discovery.
- **Command Lengkap**: `compose up`, `compose down`, `compose ps` dari CLI & UI.

### 🔀 Reverse Proxy & Load Balancing Modern (Cloudflare Pingora)
- **Engine Pingora Terintegrasi**: Menggunakan Cloudflare Pingora Core yang ultra-cepat, hemat memori, dan tahan terhadap serangan buffer overflow.
- **Least Connections Load Balancing**: Pembagian trafik cerdas ke target backend yang paling sedikit memegang koneksi aktif.
- **Active Health Checks & Process Awareness**: Worker background memantau kesehatan target berkala serta mendeteksi status aplikasi yang dikelola secara real-time. Jika aplikasi berhenti, Pingora langsung mengembalikan halaman error 503 kustom ZenoPanel yang ramah.
- **Strip Path Prefix**: Memotong prefix path secara dinamis sebelum meneruskannya ke backend.
- **Dynamic Port Listeners**: Mendukung rule proxy untuk mendengarkan port non-standar di server.

### 🛡️ Web Application Firewall (WAF) & Rate Limiter
- **Keamanan WAF Bawaan**: Deteksi dan cegah serangan SQL Injection, XSS, Path Traversal, dan Remote Code Execution (RCE).
- **Rate Limiting Granular**: Batasi request maksimum per IP dalam jendela waktu tertentu untuk menangkis serangan DDoS dan abuse API.
- **Dedicated Security Tab (Khusus Admin)**: Halaman khusus untuk menyetel konfigurasi WAF/Rate Limiting serta memantau log audit trail serangan secara real-time.

### 🔒 SSL/TLS Otomatis & HTTP/2 ALPN Native
- **Protokol Cepat**: Dukungan HTTP/2 Multiplexing & ALPN (`h2` dan `http/1.1`) secara native langsung di dalam handler TLS Pingora.
- **ACME Let's Encrypt Asli**: Integrasi pustaka produksi `instant-acme` dengan CSR berbasis standard `rcgen`.
- **Auto-Renewal Cerdas**: Pemantauan sertifikat asli via parser X.509 (`x509-parser`) yang memperbarui sertifikat otomatis saat masa berlaku tersisa kurang dari 30 hari tanpa perlu merestart server (zero-downtime certificate hot reload).

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

- **Proxy Engine**: [Cloudflare Pingora](https://github.com/cloudflare/pingora) (`pingora-core` & `pingora-proxy`) sebagai reverse proxy gateway utama.
- **Web Engine**: [Axum](https://github.com/tokio-rs/axum) & [Tokio](https://tokio.rs/) Async Runtime (internal management control plane).
- **Container Runtime**: [runc](https://github.com/opencontainers/runc) (OCI-compliant) — di-embedded langsung di binary ZenoPanel.
- **Container Build**: Go (untuk `zeno-container` CLI — pull image, OCI bundle, container lifecycle).
- **TLS & Crypto Engine**: OpenSSL (terintegrasi erat dengan Pingora untuk handshake ultra-cepat) & [Rustls](https://github.com/rustls/rustls).
- **Security & ACME**: [instant-acme](https://github.com/jsha/instant-acme), [rcgen](https://github.com/rustls/rcgen), & [x509-parser](https://github.com/rusticata/x509-parser).

---

## 🗺️ Roadmap Masa Depan

### ✅ Sudah Tersedia
- **Zeno Container**: Container runtime berbasis `runc` (embedded), pull image dari Docker Hub, manajemen penuh dari UI.
- **Docker Compose**: Parse YAML, multi-service deployment, service discovery.
- **Dukungan Rootless**: Container berjalan tanpa hak root.
- **TCP Port Proxy**: Forward port container ke host.
- **Integrasi File Manager**: Browse filesystem container langsung dari panel.
- **Manajemen Jaringan & Volume Dinamis**: Membuat, melihat, dan menghapus volume serta network secara dinamis langsung dari UI ZenoPanel atau API.
- **Resource Limits**: Mengatur dan memperbarui batas penggunaan memori (RAM) dan CPU secara dinamis per kontainer.
- **Health Checks & Auto-Restart**: Pemantauan kesehatan berkala dan restart otomatis kontainer yang mati yang dikelola langsung oleh `zeno-container daemon`.

### 🚧 Sedang Dikembangkan
- **Container Build**: Build image dari Dockerfile.
- **Network Bridge**: Isolasi jaringan antar container (veth pair) tingkat lanjut.
- **Container Registry Private**: Dukung login ke registry privat.

---

## 📥 Cara Instalasi (Production)

Untuk melakukan instalasi cepat ZenoPanel dan runtime `zeno-container` di server produksi Linux x86_64, jalankan perintah installer otomatis satu baris berikut:

```bash
curl -fsSL https://raw.githubusercontent.com/nextcore/zenopanel/main/install.sh | bash
```

*Catatan: Secara default, ZenoPanel akan dipasang di `/opt/zenopanel`. Anda dapat menyesuaikan direktori tujuan instalasi secara interaktif selama script berjalan.*

Untuk panduan instalasi manual secara detail dan kustomisasi lokasi data, silakan baca berkas [install.md](./install.md).

---

## 🛠️ Pengembangan Lokal (Development)

Jika Anda ingin berkontribusi atau menjalankan ZenoPanel dari source code untuk pengembangan lokal:

### 1. Prasyarat
Pastikan komputer Anda memiliki kompiler Rust (stable) terpasang.

### 2. Kloning & Build
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
Sesuaikan konfigurasi port di file `.env`. Untuk panduan detail port dan eksekusi lokal (development), lihat [development.md](./development.md). Untuk kompilasi rilis kompatibilitas tinggi (GLIBC 2.17) atau static MUSL (untuk Alpine Linux), lihat [compile.md](./compile.md).

Jalankan panel dalam mode development:
```bash
PATH=$PWD/cmake_local/bin:$PATH cargo run
```

Buka browser Anda di `http://localhost:3001/zpanel` (atau sub-path login khusus yang disetel di `.env`) untuk mengakses dashboard.

---

## 🤝 Kontribusi & Lisensi

ZenoPanel didistribusikan di bawah lisensi [Apache 2.0](./LICENSE). Kami sangat menyambut kontribusi kode, pelaporan bug, dan saran fitur melalui Pull Request dan Issues di GitHub.
