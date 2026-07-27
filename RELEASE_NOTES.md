# 🚀 ZenoPanel Release Notes

Daftar rilis resmi fitur, perbaikan, dan peningkatan teknologi pada platform ZenoPanel.

---

## 🌟 Versi v1.7.14 (Rilis Terbaru)

Rilis **v1.7.14** berfokus pada **perbaikan penanganan WebSocket dan koneksi HTTP Upgrade** pada port proxy kustom (dynamic ports) melalui mekanisme TCP tunneling dua arah, serta penyelarasan rilis distro WSL2.

### 🔌 1. Dukungan WebSocket & Upgrade di Port Kustom (Dynamic Listeners)
Kami membenahi penanganan koneksi WebSocket (seperti relay streaming CCTV go2rtc, WebRTC, atau MSE) yang dilewati melalui port proxy alternatif/kustom (misal: `:8888`):
- **Bypass Keterbatasan `reqwest`**: Port dinamis di luar port utama Pingora yang dilayani oleh Axum (`wildcard_handler`) kini mendeteksi request HTTP Upgrade secara eksplisit. Forwarding lama menggunakan `reqwest::Client` (yang membuang header `Connection` dan `Upgrade`) tidak digunakan lagi untuk request jenis ini.
- **TCP Tunneling & Hijacking**: ZenoPanel kini membuka koneksi TCP langsung ke upstream server untuk melakukan handshake. Jika responsnya bernilai `101 Switching Protocols`, koneksi downstream dari browser dibajak menggunakan `hyper::upgrade::on` dan ditransfer secara dua arah secara asinkron (*bidirectional copy*) menggunakan wrapper `hyper_util::rt::TokioIo`. Ini menghilangkan error `400 Bad Request` (mismatch websocket handshake) pada client streaming.

