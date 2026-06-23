# 🚀 Panduan Instalasi ZenoPanel

1. **Unduh binary ZenoPanel**:
   ```bash
   wget https://raw.githubusercontent.com/nextcore/zenopanel/main/dist/zenopanel-v0.2.1.tar.gz
   ```

2. **Ekstrak & Masuk ke Folder**:
   ```bash
   tar -xzvf zenopanel-v0.2.1.tar.gz
   cd zenopanel-v0.2.1
   ```

3. **Salin Konfigurasi Environment**:
   ```bash
   cp .env.example .env
   ```

4. **Jalankan ZenoPanel**:
   ```bash
   ./zeno
   ```

5. **Akses aplikasi di port yang ditentukan di `.env` (default: 3001)**:
   Akses melalui browser: `http://[IP_ADDRESS]:3001/login`

6. **Instalasi Service**:
   Masuk ke menu **Settings**, lalu pada bagian **Service Injector** pilih **Install** untuk mendaftarkan ZenoPanel sebagai service sistem (systemd).

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
