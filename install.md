# 🚀 Panduan Instalasi ZenoPanel

Anda dapat memasang ZenoPanel secara otomatis menggunakan script installer (Direkomendasikan) atau melakukan instalasi secara manual.

## I. Instalasi Otomatis (Direkomendasikan)

Cukup unduh dan jalankan script installer otomatis. Script ini akan memeriksa kompatibilitas sistem, mengunduh rilis tarball terbaru, memverifikasi integritas berkas menggunakan checksum SHA-256, mengekstrak aset, serta men-generate JWT key keamanan secara otomatis.

Jalankan perintah satu baris berikut di server target Anda:

```bash
curl -fsSL https://raw.githubusercontent.com/nextcore/zenopanel/main/install.sh | bash
```

*Catatan: Secara default, aplikasi akan dipasang di `/opt/zenopanel`. Anda dapat menyesuaikan direktori tujuan instalasi interaktif saat script berjalan.*

---

## II. Instalasi Manual

Jika Anda ingin mengunduh dan memasang berkas distribusi secara manual:

1. **Unduh binary ZenoPanel & Checksum**:
   ```bash
   # Unduh berkas rilis
   wget https://raw.githubusercontent.com/nextcore/zenopanel/main/dist/zenopanel-v1.0.0.tar.gz

   # Unduh berkas checksum untuk verifikasi integritas
   wget https://raw.githubusercontent.com/nextcore/zenopanel/main/dist/zenopanel-v1.0.0.tar.gz.sha256
   ```

2. **Verifikasi Berkas & Ekstrak**:
   ```bash
   # Verifikasi integritas berkas tarball
   sha256sum -c zenopanel-v1.0.0.tar.gz.sha256
   # Hasil yang diharapkan: zenopanel-v1.0.0.tar.gz: OK

   # Ekstrak dan masuk ke folder
   tar -xzvf zenopanel-v1.0.0.tar.gz
   cd zenopanel-v1.0.0
   ```

3. **Salin Konfigurasi Environment & Inisialisasi**:
   ```bash
   cp .env.example .env
   
   # Jalankan generator JWT_SECRET jika menggunakan binary baru
   ./zeno key:generate
   ```

4. **Jalankan ZenoPanel**:
   ```bash
   ./zeno
   ```

5. **Akses Aplikasi**:
   Akses melalui browser di port yang ditentukan di `.env` (default: `3001`):
   `http://[IP_ADDRESS]:3001/login`

6. **Instalasi Service (Systemd)**:
   Masuk ke menu **Settings** di dashboard, lalu pada bagian **Service Injector** pilih **Install** untuk mendaftarkan ZenoPanel sebagai service sistem (systemd).

---

## 🐳 Panduan & Lokasi `zeno-container` (Container Runtime)

ZenoPanel dilengkapi dengan runtime container berbasis OCI (`runc`) yang dikelola oleh binary pendukung bernama `zeno-container`.

### Otomatisasi Instalasi (Direkomendasikan)
Ketika Anda melakukan instalasi service systemd (Langkah 6 di atas) via dashboard ZenoPanel, sistem akan berjalan sebagai **root** dan secara otomatis mendeteksi keberadaan file binary `zeno-container` di dalam folder ZenoPanel (seperti di `./zeno-container` atau di subfolder `./modul/zeno-container`). 

Jika terdeteksi, ZenoPanel akan otomatis:
1. Menyalin binary ke `/usr/local/bin/zeno-container`.
2. Memberikan izin eksekusi (`chmod +x`).
3. Membuat direktori penyimpanan data `/var/lib/zeno-container`.

### Lokasi Default Sistem
* **Binary Executable**: `/usr/local/bin/zeno-container`  
  *Alasan*: Agar dapat dieksekusi secara global langsung dari terminal oleh administrator server.
* **Direktori Data (State & Storage)**: `/var/lib/zeno-container`  
  *Alasan*: Menyimpan data dinamis (image, volume mount, bundle container) secara persisten di lokasi sistem yang aman dari risiko terhapus secara tidak sengaja ketika Anda menghapus atau memperbarui folder aplikasi ZenoPanel.

### Kustomisasi Lokasi
Jika Anda ingin memindahkan letak binary atau data container ke lokasi lain, Anda cukup menambahkan atau menyesuaikan variabel berikut di file konfigurasi `.env` Anda:

```env
# Jalur ke executable file binary zeno-container
ZENO_CONTAINER_BIN=/jalur/kustom/ke/zeno-container

# Direktori penyimpanan data/state container
ZENO_CONTAINER_DATA_DIR=/jalur/kustom/data/zeno-container
```
