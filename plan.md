# Rencana Implementasi: Sisa Peningkatan ZenoPanel sebagai Pengganti Nginx / Caddy

Dokumen ini mendefinisikan peta jalan (roadmap) dan arsitektur teknis yang **belum tercapai** untuk meningkatkan performa, keamanan, dan fitur reverse proxy ZenoPanel agar setara dengan standar produksi industri seperti Nginx dan Caddy.

---

## 1. Tujuan Utama (Goal)
Melanjutkan peningkatan sub-sistem reverse proxy ZenoPanel dengan fokus pada dukungan WebSocket secara native, HTTP/2 multiplexing, optimalisasi penyajian file statis tingkat kernel (zero-copy), algoritma load balancing tingkat lanjut (Least Connections & Active Health Checks), serta mitigasi serangan DDoS.

---

## 2. Kebutuhan Teknis & Fitur Baru (Belum Tercapai)

### Fasa 1 (Tambahan): Dukungan Protokol WebSocket (`ws://` dan `wss://`)
*   **Masalah**: Koneksi real-time berbasis WebSocket saat ini belum didukung karena forwarder stream HTTP biasa tidak menangani upgrade request.
*   **Solusi**:
    *   Deteksi header `Upgrade: websocket` dan `Connection: Upgrade` pada `wildcard_handler`.
    *   Gunakan mekanisme upgrade koneksi Axum (`axum::extract::WebSocketUpgrade`) untuk membajak (hijack) TCP socket client dan membuat koneksi TCP paralel ke backend target, lalu menjembatani data stream secara bidireksional.

### Fasa 2: Protokol HTTP/2 & Negosiasi TLS (ALPN)
*   **Masalah**: Server hanya melayani HTTP/1.1 yang lambat untuk banyak request aset paralel.
*   **Solusi**:
    *   Mengonfigurasi listener Axum menggunakan **`tokio-rustls`** dengan dukungan **ALPN** (h2 dan http/1.1).
    *   Mendukung multiplexing bawaan HTTP/2 sehingga browser dapat mengunduh banyak aset dalam satu koneksi TCP tunggal.

### Fasa 3: Optimalisasi Penyajian File Statis
*   **Masalah**: Penanganan file statis membebani CPU/RAM untuk situs dengan traffic tinggi.
*   **Solusi**:
    *   Menggunakan fitur **`tokio::fs::File`** yang efisien atau memanggil system call `sendfile` secara langsung untuk pemindahan data tingkat kernel (*zero-copy*).
    *   Mendukung **Brotli & Gzip Pre-compression**: Jika browser mengirimkan `Accept-Encoding: br`, server akan mencari file terkompresi terlebih dahulu (misal `index.html.br`) untuk menghemat bandwidth.
    *   Penyisipan otomatis header `Cache-Control` (Max-Age, Immutable) dan validasi `ETag` / `If-None-Match`.

### Fasa 4 (Lanjutan): Least Connections & Active Health Checks
*   **Masalah**: Pembagian beban baru mendukung Round-Robin tanpa melacak koneksi aktif maupun memantau apakah backend target sedang mati (down).
*   **Solusi**:
    *   **Least Connections**: Lacak jumlah request aktif per target URL secara in-memory menggunakan `Arc<Mutex<HashMap<String, usize>>>`, dan arahkan request baru ke target dengan koneksi aktif paling sedikit.
    *   **Active Health Checks**: Tambahkan *background thread* berkala (misal tiap 5 detik) untuk melakukan HTTP GET request (ping) ke setiap target backend. Jika target gagal merespons, tandai sebagai tidak sehat (*unhealthy*) dan keluarkan dari kolam routing load balancer hingga kembali sehat.

### Fasa 5: Keamanan & Rate Limiting (DDoS Mitigation)
*   **Masalah**: Tidak ada perlindungan terhadap traffic anomali di port proxy publik.
*   **Solusi**:
    *   Mengintegrasikan **`tower-limit`** untuk membatasi jumlah request per unit waktu berbasis alamat IP client (Token Bucket).
    *   Menambahkan pembatasan koneksi aktif serentak per IP untuk mencegah eksploitasi Slowloris.

---

## 3. Rencana Perubahan Kode (Proposed Changes)

### [MODIFY] `Cargo.toml`
*   Tambahkan dependensi: `tokio-rustls`, `tower-http` (fitur limit), `brotli`, `gzip`.

### [MODIFY] [`src/proxyman.rs`](file:///home/max/Documents/PROJ/github/zenopanel/src/proxyman.rs)
*   Implementasikan struktur data pelacak koneksi aktif (`Least Connections`).
*   Tambahkan modul pemantau kesehatan backend (*active health checker*) berbasis task background tokio.

### [MODIFY] [`src/main.rs`](file:///home/max/Documents/PROJ/github/zenopanel/src/main.rs)
*   Integrasikan deteksi & upgrade WebSocket di `wildcard_handler`.
*   Konfigurasi TCP listener agar mendukung negosiasi TLS ALPN untuk HTTP/2.

---

## 4. Rencana Pengujian & Validasi (Verification Plan)

### Pengujian Otomatis (Automated Tests)
1.  **Uji WebSocket**: Buat aplikasi WebSocket server sederhana di backend, lalu kirim pesan client via ZenoPanel port proxy dan pastikan komunikasi dua arah berjalan lancar.
2.  **Uji Active Health Check**: Jalankan dua target, hentikan salah satunya, dan pastikan load balancer langsung mendeteksi dalam waktu < 5 detik lalu mengalihkan seluruh traffic ke target yang tersisa.
3.  **Uji Rate Limiting**: Kirim request bertubi-tubi (misal 100 request dalam 1 detik) dari alamat IP yang sama, dan pastikan server membalas dengan status `429 Too Many Requests`.

### Pengujian Manual
*   Buka halaman web menggunakan Google Chrome, tekan F12 (Inspect Element) -> Network Tab, dan pastikan kolom *Protocol* menunjukkan `h2` (HTTP/2) bukan `http/1.1`.


Task
# Auto SSL Let's Encrypt Integration Checklist

## Tasks

- [ ] Tambahkan `instant-acme` ke `Cargo.toml`
- [ ] Implementasikan alur pembuatan/pemuatan kunci akun ACME di `./certs/acme_account.key`
- [ ] Refaktor `trigger_acme_flow` di `src/sslman.rs` menggunakan `instant-acme`
    - [ ] Hubungkan ke Let's Encrypt Staging / Production sesuai env `SSL_PRODUCTION`
    - [ ] Buat order baru untuk domain
    - [ ] Selesaikan tantangan HTTP-01 dan masukkan token ke map `ACME_CHALLENGES`
    - [ ] Lakukan polling status validasi tantangan
    - [ ] Buat CSR dengan `rcgen` dan finalisasi order
    - [ ] Unduh sertifikat PEM dan simpan di `./certs/{domain}.crt` dan `.key`
    - [ ] Hapus cache in-memory dan update database
- [ ] Verifikasi kompilasi dan perbaiki error tipe data
- [ ] Jalankan pengujian integrasi di lingkungan lokal

Implementation Plan
# Rencana Implementasi: Integrasi Let's Encrypt Auto SSL Asli

Rencana ini merinci langkah-langkah teknis untuk mengganti sistem SSL simulasi di ZenoPanel dengan sistem **Auto SSL asli** berbasis protokol ACME v2 menggunakan Let's Encrypt (Staging & Production) dan pustaka `instant-acme`.

---

## User Review Required

> [!IMPORTANT]
> **Tingkat Keberhasilan Verifikasi DNS & Port**:
> Agar Let's Encrypt dapat memverifikasi kepemilikan domain via tantangan HTTP-01:
> 1. Domain publik Anda harus sudah memiliki DNS A Record yang mengarah ke IP publik server Anda.
> 2. ZenoPanel harus berjalan di port standar HTTP 80 (untuk tantangan Let's Encrypt) dan port HTTPS 443.
> 3. Jika diuji secara lokal (tanpa domain publik asli), sistem akan otomatis mendeteksi kegagalan koneksi publik Let's Encrypt dan jatuh kembali (*fallback*) ke sertifikat *self-signed*, sehingga jalannya aplikasi tetap aman secara lokal.

---

## Proposed Changes

### 1. Dependensi (`Cargo.toml`)
*   Menambahkan pustaka **`instant-acme`** untuk menangani protokol ACME v2 secara asinkron.

### 2. Modul SSL Manager (`src/sslman.rs`)
Refaktorisasi alur `trigger_acme_flow` dengan langkah berikut:
*   **Penyimpanan Kunci Akun**: Menyimpan/memuat kunci privat akun Let's Encrypt di `./certs/acme_account.key` agar tidak membuat akun baru setiap kali server dinyalakan ulang.
*   **Pemilihan Environment (Staging vs Production)**: Mendeteksi variabel lingkungan `SSL_PRODUCTION=true`. Jika tidak ada, gunakan Let's Encrypt Staging agar tidak terkena *rate limit* saat testing.
*   **Inisiasi ACME Order**: Membuat order sertifikat baru untuk domain target.
*   **Fulfill HTTP-01 Challenge**:
    *   Mendapatkan `token` dan `key_authorization` dari Let's Encrypt.
    *   Memasukkannya ke dalam map global `ACME_CHALLENGES`.
    *   Memanggil `.ready()` untuk memberi tahu Let's Encrypt untuk melakukan validasi.
*   **Polling Status**: Melakukan polling status order secara asinkron dengan jeda waktu berkala hingga statusnya valid.
*   **CSR Generation (Pembangkitan CSR)**: Menggunakan `rcgen` untuk membuat Certificate Signing Request (CSR) berbasis kunci privat baru untuk domain, lalu mengirimkannya ke Let's Encrypt via `.finalize()`.
*   **Unduh Sertifikat**: Setelah order ditandatangani, unduh rantai sertifikat PEM penuh dan simpan di `./certs/{domain}.crt` dan `./certs/{domain}.key`.
*   **Pembersihan Cache**: Menghapus cache memori sertifikat lama di `ZenoCertResolver` dan mengupdate kolom status database menjadi `"active_letsencrypt"`.

---

## Verification Plan

### Automated/Integration Tests
1.  **Pengujian Alur Fallback Lokal**: Jalankan pengujian di mesin lokal tanpa domain publik, verifikasi bahwa kegagalan validasi ACME ditangani dengan anggun dan otomatis beralih ke pembuatan sertifikat *self-signed* agar proxy lokal tetap berjalan aman.
2.  **Uji Integrasi Skrip**:
    *   Buat skrip `scratch/test_real_ssl.py` yang memicu registrasi SSL untuk domain lokal/uji dan memverifikasi transisi status database dari `pending` -> `active_self_signed` atau `active_letsencrypt`.

### Manual Verification
*   Jalankan server ZenoPanel di server VPS publik dengan domain asli, aktifkan SSL untuk domain tersebut lewat ZenoPanel, dan periksa apakah browser mendapatkan sertifikat valid yang diterbitkan oleh Let's Encrypt Authority.

