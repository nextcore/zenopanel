# 🛠️ Panduan Kompilasi ZenoPanel

Kompilasi ZenoPanel kini telah diotomatisasi secara penuh menggunakan script kompilasi terpadu [compile.sh](./compile.sh). Script ini bertugas membersihkan cache, melakukan kompilasi silang (cross-compilation) untuk Rust secara statis, mengoptimalkan ukuran binary dengan pemotongan debug symbol (*stripped*), serta mengemas berkas distribusi beserta checksum SHA-256 ke dalam direktori `dist/`.

---

## I. Menggunakan Script Kompilasi (`compile.sh`)

Anda dapat menjalankan script ini dalam dua mode: **Interaktif** (dengan panduan prompt) atau **Non-Interaktif** (menggunakan parameter baris perintah).

### 1. Mode Interaktif (Direkomendasikan untuk Penggunaan Manual)
Cukup jalankan script tanpa argumen tambahan:
```bash
./compile.sh
```
Script akan menanyakan secara bertahap:
- Target kompilasi (`musl` sebagai default, atau `gnu`).
- Versi paket rilis (otomatis mendeteksi versi Git tag terakhir, misal `v1.0.1`).
- Apakah ingin membersihkan cache build (`cargo clean`) terlebih dahulu.

### 2. Mode Non-Interaktif (Cocok untuk CI/CD atau Scripting)
Gunakan flag `--non-interactive` atau `-y` bersamaan dengan parameter kustom:
```bash
# Jalankan kompilasi bersih untuk versi v1.0.0 dengan target MUSL secara otomatis
./compile.sh --non-interactive --target musl --version v1.0.1 --clean
```

---

## II. Daftar Opsi & Argumen CLI yang Didukung

| Opsi / Parameter | Deskripsi |
| :--- | :--- |
| `--non-interactive`, `-y` | Menjalankan build secara langsung tanpa memicu prompt pertanyaan interaktif. |
| `--target [musl\|gnu]` | Menentukan target C Runtime (libc) pada Linux:<br>• `musl` (Default): Kompilasi statis penuh murni (cocok untuk Alpine Linux).<br>• `gnu`: Kompilasi kompatibilitas mundur hingga **GLIBC 2.17** (kompatibel dengan CentOS 7/Ubuntu 14.04+). |
| `--version [versi]` | Mengubah label versi paket tarball dan rilis final (misal: `v1.0.0`). |
| `--clean` | Menghapus folder `target/` Rust sebelum proses compile untuk menjamin hasil build bersih. |
| `--help`, `-h` | Menampilkan panduan bantuan CLI. |

---

## III. Detail Target Kompilasi

### 1. Target `musl` (Default)
Mengompilasi binary secara statis murni sehingga tidak bergantung pada library dinamis host pembangun maupun host target. Sangat cocok untuk berjalan langsung di **Alpine Linux** maupun distro Linux modern lainnya (seperti Ubuntu, Debian, Rocky Linux) tanpa kendala pustaka sistem (*zero dependency*).

### 2. Target `gnu` (GLIBC 2.17)
Menggunakan compiler backend **Zig** untuk menghubungkan library sistem target. Memungkinkan binary berjalan di kernel/sistem Linux yang lebih tua yang masih menggunakan sistem operasi berbasis GLIBC lawas (seperti CentOS 7 lama).

---

## IV. Hasil Akhir & Verifikasi

Setelah kompilasi selesai, direktori [dist/](./dist) akan terisi berkas paket rilis:

1. **Tarball Distribusi**: `dist/zenopanel-<versi>.tar.gz`
2. **Berkas Checksum**: `dist/zenopanel-<versi>.tar.gz.sha256`

### Uji Integritas & Verifikasi Struktur Binary
Gunakan perintah `file` untuk melihat karakteristik binary yang berada di dalam paket:

```bash
# Mengekstrak sementara untuk verifikasi
tar -xzf dist/zenopanel-v1.0.1.tar.gz

# Periksa status penautan binary
file zenopanel-v1.0.1/zeno
```

**Output yang Diharapkan:**
```text
zeno: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked, stripped
```
Keterangan `statically linked` dan `stripped` menandakan binary mandiri penuh (tidak membutuhkan library luar) dan ukurannya telah dioptimalkan secara maksimal.
