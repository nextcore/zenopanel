# 🚀 ZenoPanel Release Notes

Daftar rilis resmi fitur, perbaikan, dan peningkatan teknologi pada platform ZenoPanel.

---

## 🌟 Versi v1.8.2 (Rilis Terbaru)

Rilis **v1.8.2** menghadirkan peningkatan antarmuka pengguna (UI/UX) dengan tombol slide toggle sidebar dinamis, dukungan penuh **Host Network Mode** pada engine Zeno Box Compose, manajemen database ProxySQL connection pooling, serta optimasi sistem Firewall bawaan dengan lockout protection.

---

### 🎨 1. Sidebar Slide Toggle & Collapsible Layout (UI/UX)
- **Desain Responsif Fleksibel**: Menambahkan tombol *slide toggle* pada sidebar utama ZenoPanel untuk merubah tampilan dari lebar penuh (`260px`) menjadi mode ringkas ikon (`76px`).
- **Animasi CSS Halus & Smart Tooltips**: Transisi lebar sidebar dan pemosisian ikon dibuat sangat responsif dilengkapi *tooltip* judul menu saat sidebar terlipat.
- **LocalStorage State Persistence**: Status sidebar (*expanded* atau *collapsed*) tersimpan secara otomatis di peramban pengguna dan bertahan saat navigasi antar halaman.

### 🐳 2. Zeno Box Engine & Docker Compose Enhancements
- **Dukungan `network_mode: host` di Compose**: Engine Zeno Box Compose kini secara resmi mendukung deklarasi `network_mode: host` dari berkas `docker-compose.yml`, memungkinkan kontainer mengakses antarmuka jaringan fisik host tanpa hambatan NAT.
- **Perbaikan Parsing Environment Variables**: Penanganan variabel lingkungan (*env*) untuk kontainer Zeno Box kini mendukung format objek JSON map (`{"KEY": "VALUE"}`).

### 🗄️ 3. Integrasi ProxySQL & Database Connection Pooling
- **Generator Konfigurasi ProxySQL**: Integrasi slot ZenoLang `db.generate_proxysql_config` untuk mempermudah *deployment* ProxySQL Connection Pooling.
- **Manajemen Port & Hostgroup Dinamis**: Memungkinkan penerusan query SQL berkecepatan tinggi dari port standar `3306` ke backend database terisolasi.

### 🛡️ 4. Firewall Rules Manager & Lockout Protection (iptables)
- **Penyelarasan Direct iptables Kernel**: Penambahan dan penghapusan aturan firewall via API `/api/security/firewall` disinkronkan secara *real-time* ke kernel `iptables` Linux.
- **Fitur Lockout Protection**: Proteksi otomatis agar port manajemen vital (SSH `22`, HTTP `80/443`, dan port ZenoPanel) tidak dapat terblokir secara tidak sengaja.

---

### 🛠️ 5. Perbaikan Bug & Optimasi (Bug Fixes & Maintenance)
- **Matchit 0.8 Path Syntax**: Penyesuaian sintaks routing ZenoEngine ke format parameter `{param}`.
- **CSRF Token Handling**: Perbaikan ekstraksi header `X-CSRF-Token` pada login JSON API.
- **Tampilan Header Mobile & Desktop**: Tombol toggle universal `toggleSidebar()` disinkronkan untuk perangkat seluler maupun komputer meja.

---

### 📦 6. Informasi Distribusi

| Artefak | Ukuran | Keterangan |
|---|---|---|
| `zenopanel-v1.8.2.tar.gz` | ~26 MB | Static binary Linux (x86_64-musl) |

---
