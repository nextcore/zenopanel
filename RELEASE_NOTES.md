# ZenoPanel v1.9.0 Release Notes 🚀

**Release Date:** July 31, 2026  
**Tag:** `v1.9.0`  
**Distribution Bundle:** `zenopanel-v1.9.0.tar.gz`

---

## 🌟 Major Feature: Native Docker Container Import to Zeno Box

ZenoPanel v1.9.0 memperkenalkan kemampuan untuk **mengimpor kontainer Docker yang sedang atau pernah berjalan di host** langsung ke engine native **Zeno Box** (`runc` + OCI standard).

### 🚀 Key Highlights & Capabilities:
- **Zero-Downtime Migration**: Mengimpor metadata (Environment Variables, Entrypoint/CMD, Working Directory, Exposed Ports, Mounts) dan isi berkas kontainer Docker.
- **Dual Import Modes**:
  1. **Single Container (RootFS Snapshot)**: Melakukan snapshot dan ekstraksi filesystem penuh kontainer Docker ke OCI RootFS Zeno Box. Cocok untuk aplikasi *stateless* atau *standalone*.
  2. **Zeno Box Compose Project**: Menggenerasi file `docker-compose.yml` secara otomatis dan menempatkannya di tab **Compose**, siap dikelola via Monaco Code Editor.
- **Zero-Copy Instant Volume Preservation (Optimized for Large Databases)**:
  - Opsi *Zero-Copy* untuk direktori data / database besar (seperti MySQL, PostgreSQL, Redis).
  - Menggunakan *Direct Bind Mount* tanpa menyalin file puluhan GB, sehingga proses impor selesai **dalam 0 detik** tanpa memakan ruang disk tambahan.

---

## ⚡ ProxySQL & High-Concurrency Database Enhancements

- **Optimasi ProxySQL Sidecar**: Perbaikan konfigurasi *Network Mode* pada Zeno Box Compose untuk mendukung koneksi ProxySQL backend pool dengan latency rendah dan pencegahan *hairpin loop*.

---

## 🛠️ API & Engine Changes

### New ZenoLang Engine Slots (`src/slots/zeno_box/container.rs`):
- `system.list_docker_containers`: Mengambil daftar kontainer Docker di host via daemon socket/CLI.
- `box.import_docker`: Slot backend untuk memproses ekstraksi `docker inspect` & `docker export` atau penyusunan berkas Compose.

### New API Endpoints (`zsrc/routes/containers.zl`):
- `GET /api/containers/docker-list`: Mendapatkan daftar kontainer Docker di host.
- `POST /api/containers/import-docker`: Memicu alur pengimporan kontainer.

---

## 🎨 UI / UX Improvements
- **Import Docker Modal** pada tab *Containers*:
  - Dropdown interaktif dengan pencarian kontainer Docker lokal.
  - Auto-fill saran nama kontainer Zeno Box baru.
  - Opsi pemilihan mode impor (Container vs Compose) dan toggle *Zero-Copy Volume Mount*.

---
