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
