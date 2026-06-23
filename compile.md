# 🛠️ Panduan Kompilasi ZenoPanel & `zeno-container`

Panduan ini menjelaskan cara mengompilasi ZenoPanel dan runtime `zeno-container` dengan kompatibilitas tinggi (retro-compatible ke GLIBC 2.17) maupun sebagai static binary murni untuk target **MUSL** (misal untuk Alpine Linux atau OS minimalis tanpa libc dinamis).

---

## I. Kompilasi Kompatibilitas Tinggi (Target GLIBC 2.17)

Kompilasi bawaan Rust (`cargo build --release`) menghasilkan binary yang bergantung pada versi GLIBC mesin host pembangun (misalnya GLIBC 2.35+ di Ubuntu modern). Jika dijalankan di server lama, hal ini akan memicu error *GLIBC version not found*.

Untuk mengatasinya, kita menggunakan **compiler Zig** sebagai backend linker dan compiler C/C++ agar dapat mengompilasi binary target `x86_64-unknown-linux-gnu` dengan kompatibilitas mundur hingga **GLIBC 2.17** (kompatibel dengan CentOS 7, Ubuntu 14.04+, Debian 8+, dll).

### Langkah-Langkah Kompilasi:

1. **Buat Script Wrapper Compiler**  
   Buat script-script berikut di dalam folder `zig_wrappers/`:
   
   * `zig-wrapper.sh`
   * `zig-cc.sh`
   * `zig-cxx.sh`
   * `zig-ar.sh`
   * `zig-linker-wrapper.sh`

2. **Berikan Izin Eksekusi**
   ```bash
   chmod +x zig_wrappers/*.sh
   ```

3. **Jalankan Kompilasi GLIBC 2.17**
   ```bash
   ZIG_TARGET="x86_64-linux-gnu.2.17" \
   CC_x86_64_unknown_linux_gnu="$PWD/zig_wrappers/zig-cc.sh" \
   CXX_x86_64_unknown_linux_gnu="$PWD/zig_wrappers/zig-cxx.sh" \
   AR_x86_64_unknown_linux_gnu="$PWD/zig_wrappers/zig-ar.sh" \
   CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$PWD/zig_wrappers/zig-linker-wrapper.sh" \
   cargo build --release --target x86_64-unknown-linux-gnu
   ```

   Binary hasil kompilasi akan berada di:  
   `target/x86_64-unknown-linux-gnu/release/zeno`

---

## II. Kompilasi Target Static MUSL

Jika Anda ingin menghasilkan static binary murni tanpa ketergantungan dinamis terhadap libc sistem host (sangat cocok untuk Alpine Linux atau container minimalis):

### 1. Kompilasi ZenoPanel (Rust)

Pastikan target target musl terpasang pada Rust:
```bash
rustup target add x86_64-unknown-linux-musl
```

Jalankan kompilasi menggunakan Zig wrapper yang diarahkan ke target MUSL, serta menonaktifkan link runtime internal bawaan Rust agar tidak terjadi konflik symbol:

```bash
RUSTFLAGS="-C link-self-contained=no" \
ZIG_TARGET="x86_64-linux-musl" \
CC_x86_64_unknown_linux_musl="$PWD/zig_wrappers/zig-cc.sh" \
CXX_x86_64_unknown_linux_musl="$PWD/zig_wrappers/zig-cxx.sh" \
AR_x86_64_unknown_linux_musl="$PWD/zig_wrappers/zig-ar.sh" \
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$PWD/zig_wrappers/zig-linker-wrapper.sh" \
cargo build --release --target x86_64-unknown-linux-musl
```

- **`RUSTFLAGS="-C link-self-contained=no"`**: Menginstruksikan Rust untuk tidak menyertakan objek `crt` (C Runtime) bawaan miliknya agar tidak bentrok dengan crt yang disuplai oleh Zig linker (menghindari error *duplicate symbol _start*).
- **Target-specific env vars (seperti `CC_x86_64_...`)**: Menjamin hanya library target saja yang dikompilasi menggunakan Zig, sementara host proc-macro (seperti `sqlx-macros`) tetap memakai host toolchain bawaan.

Binary hasil kompilasi akan berada di:  
`target/x86_64-unknown-linux-musl/release/zeno`

---

### 2. Kompilasi `zeno-container` (Go)

Bahasa pemrograman Go mempermudah pembuatan static binary dengan menonaktifkan integrasi CGO (`CGO_ENABLED=0`). Saat dinonaktifkan, runtime Go akan memanggil kernel syscall secara langsung melalui bahasa rakitan (assembly) tanpa memerlukan perantara libc dinamis (baik GLIBC maupun MUSL).

Jalankan perintah berikut untuk mengompilasi static binary `zeno-container` untuk arsitektur Linux 64-bit:

```bash
# Masuk ke folder modul
cd modul/zeno-container

# Kompilasi dengan menonaktifkan CGO dan memotong debug symbol (untuk memperkecil ukuran file)
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -ldflags "-s -w" -o zeno-container ./cmd/zeno-container/
```

Binary hasil kompilasi static `zeno-container` akan berada langsung di folder tersebut. Anda dapat langsung menggunakannya di server target (GLIBC atau MUSL/Alpine) secara langsung.

---

## 🔍 Cara Verifikasi Hasil Kompilasi

Untuk memastikan binary benar-benar tertaut secara statis tanpa ketergantungan library luar:

```bash
file target/x86_64-unknown-linux-musl/release/zeno
file modul/zeno-container/zeno-container
```

**Output yang Diharapkan:**
```text
statically linked, stripped
```
Jika file dideskripsikan sebagai `statically linked`, maka binary dipastikan mandiri penuh dan siap didistribusikan ke server Linux mana pun.
