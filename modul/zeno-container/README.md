# zeno-container

**Lightweight container runtime manager** — bagian dari ekosistem **ZenoPanel**.

Menggunakan `runc` (OCI runtime) untuk menjalankan container tanpa Docker daemon. Image di-pull langsung dari registry via **Docker Registry HTTP API V2** tanpa dependensi berat.

## 🔧 Prasyarat

- **Go** 1.21+ (untuk build)
- **runc** — [install runc](https://github.com/opencontainers/runc) di sistem host
- **Linux** (namespace support)
- **Root privileges** (runc butuh root untuk namespace isolation)

## 📦 Build

```bash
cd modul/zeno-container
go build -o zeno-container ./cmd/zeno-container/
```

Static binary (size ~9MB):
```bash
CGO_ENABLED=0 go build -o zeno-container ./cmd/zeno-container/
```

## 🚀 Usage

```bash
# Pull image (tanpa Docker!)
zeno-container --data-dir /tmp/zeno-test pull nginx:alpine
zeno-container --data-dir /tmp/zeno-test pull node:18-alpine

# Buat container (butuh root untuk runc)
sudo zeno-container create my-nginx --image nginx:alpine --port 8080:80

# Start container
sudo zeno-container start my-nginx

# Lihat daftar container
zeno-container ps
zeno-container ps --json   # output JSON (untuk ZenoPanel API)

# Stop & hapus
sudo zeno-container stop my-nginx
sudo zeno-container rm my-nginx

# Lihat image yang ter-cache
zeno-container images
```

## 🏗️ Arsitektur

```
┌──────────────────────────────────────┐
│           zeno-container CLI          │
│  pull │ create │ start │ stop │ rm   │
└──────┬────────────────────────┬──────┘
       │                        │
       ▼                        ▼
┌──────────────┐       ┌──────────────┐
│  Registry    │       │    runc      │
│  API V2      │       │  (OCI spec)  │
│  (pull image)│       │  (container) │
└──────────────┘       └──────────────┘
       │                        │
       ▼                        ▼
┌──────────────────────────────────────┐
│         /var/lib/zeno-container      │
│  containers/  images/  runc/        │
└──────────────────────────────────────┘
```

## 🔌 Integrasi dengan ZenoPanel

Binary ini dirancang untuk dijalankan sebagai **managed process** oleh ZenoPanel:

```bash
# Di ZenoPanel Process Manager:
# Name: Container Runtime
# Command: /usr/local/bin/zeno-container daemon --socket /tmp/zeno-container.sock
# Auto-restart: true
```

ZenoPanel akan memanggil `zeno-container ps --json` untuk mendapatkan daftar container, dan command CLI lainnya untuk lifecycle management.

## 📁 Data Directory Structure

```
/var/lib/zeno-container/
├── containers/
│   ├── <container-id>/
│   │   ├── state.json         # Container state
│   │   └── bundle/
│   │       ├── config.json    # OCI runtime spec
│   │       └── rootfs/        # Container filesystem
│   └── ...
├── images/
│   ├── library_nginx_alpine/  # Cached image layers
│   │   ├── rootfs/            # Extracted filesystem
│   │   ├── image-config.json  # Image config (CMD, ENV, etc.)
│   │   └── *.tar.gz           # Compressed layers
│   └── ...
└── runc/                      # runc state
```

## ⚖️ Perbandingan dengan Docker

| Aspek | Docker Daemon | zeno-container |
|---|---|---|
| **Binary size** | 50+ MB | **~9 MB** |
| **RAM idle** | ~100-200 MB | **~5 MB** (hanya saat dipanggil) |
| **Dependency** | Docker CE | **runc** saja |
| **Image pull** | Docker Engine API | **Registry API V2 langsung** |
| **Namespace** | via runc | via runc |
| **Network** | bridge/default | **None** (butuh konfig manual) |
| **Logging** | json-file | **Belum** (stdout/stderr capture via ZenoPanel) |
