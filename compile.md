# 🛠️ Panduan Kompilasi ZenoPanel Kompatibilitas Tinggi (Target GLIBC 2.17)

Kompilasi bawaan Rust (`cargo build --release`) menghasilkan binary yang bergantung pada versi GLIBC mesin host pembangun (misalnya GLIBC 2.35+ di Ubuntu modern). Jika dijalankan di server lama, hal ini akan memicu error *GLIBC version not found*.

Untuk mengatasinya, kita menggunakan **compiler Zig** sebagai backend linker dan compiler C/C++ agar dapat mengompilasi binary target `x86_64-unknown-linux-gnu` dengan kompatibilitas mundur hingga **GLIBC 2.17** (kompatibel dengan CentOS 7, Ubuntu 14.04+, Debian 8+, dll).

Metode ini terbukti berhasil menghindari error perakitan assembly pada crate kriptografi (seperti `aws-lc-sys`) yang biasanya terjadi ketika memaksa kompilasi ke target MUSL murni menggunakan Zig.

---

## Langkah-Langkah Kompilasi

### 1. Prasyarat
Pastikan **Zig** dan **Rust** sudah terinstal pada mesin pembangun Anda:
- `rustc --version`
- `zig version` (direkomendasikan versi 0.10 ke atas)

---

### 2. Buat Script Wrapper Compiler
Karena Cargo/cc-rs mengirimkan argument target dalam format Rust (`x86_64-unknown-linux-gnu`) yang tidak dipahami oleh Zig, kita membutuhkan script pembungkus (*wrapper*) untuk menerjemahkannya ke format target Zig (`x86_64-linux-gnu.2.17`).

Buat script-script berikut di dalam folder `target/` (folder ini diabaikan oleh Git):

#### A. Buat pembungkus utama: `target/zig-wrapper.sh`
```bash
#!/bin/bash
CMD="$1"
shift

args=()
skip=0
for ((i=1; i<=$#; i++)); do
    if [ $skip -gt 0 ]; then
        skip=$((skip - 1))
        continue
    fi
    arg="${!i}"
    if [[ "$arg" == --target=* ]]; then
        continue
    elif [[ "$arg" == "--target" ]] || [[ "$arg" == "-target" ]]; then
        skip=1
        continue
    else
        args+=("$arg")
    fi
done

if [ "$CMD" = "ar" ]; then
    exec zig ar "${args[@]}"
elif [ "$CMD" = "c++" ]; then
    exec zig c++ -target x86_64-linux-gnu.2.17 "${args[@]}"
else
    exec zig cc -target x86_64-linux-gnu.2.17 "${args[@]}"
fi
```

#### B. Buat pembungkus untuk CC: `target/zig-cc.sh`
```bash
#!/bin/bash
exec $(dirname "$0")/zig-wrapper.sh cc "$@"
```

#### C. Buat pembungkus untuk CXX: `target/zig-cxx.sh`
```bash
#!/bin/bash
exec $(dirname "$0")/zig-wrapper.sh c++ "$@"
```

#### D. Buat pembungkus untuk AR: `target/zig-ar.sh`
```bash
#!/bin/bash
exec $(dirname "$0")/zig-wrapper.sh ar "$@"
```

#### E. Buat pembungkus Linker Cargo: `target/zig-linker-wrapper.sh`
```bash
#!/bin/bash
exec $(dirname "$0")/zig-wrapper.sh cc "$@"
```

---

### 3. Berikan Izin Eksekusi pada Semua Script
Jalankan perintah berikut untuk mengizinkan eksekusi semua script wrapper:
```bash
chmod +x target/zig-wrapper.sh target/zig-cc.sh target/zig-cxx.sh target/zig-ar.sh target/zig-linker-wrapper.sh
```

---

### 4. Jalankan Kompilasi
Eksekusi kompilasi release dengan menyuntikkan wrapper script ke environment compiler Cargo:

```bash
CC="./target/zig-cc.sh" \
CXX="./target/zig-cxx.sh" \
AR="./target/zig-ar.sh" \
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="./target/zig-linker-wrapper.sh" \
cargo build --release
```

Binary hasil kompilasi akan berada di:
`target/release/zeno`

---

## 🔍 Cara Verifikasi Hasil Kompilasi
Untuk memastikan binary yang dihasilkan benar-benar hanya menggunakan simbol GLIBC versi lama, jalankan perintah berikut pada terminal:

```bash
objdump -p target/release/zeno | grep -E 'GLIBC_' | sort -u
```

**Hasil yang Benar:**
Output tidak boleh memuat versi GLIBC yang lebih tinggi dari `GLIBC_2.17`. Contoh output yang valid:
```text
GLIBC_2.2.5
GLIBC_2.3
GLIBC_2.3.2
GLIBC_2.3.4
GLIBC_2.4
GLIBC_2.7
GLIBC_2.9
GLIBC_2.10
GLIBC_2.12
GLIBC_2.15
GLIBC_2.16
GLIBC_2.17
```
Jika tidak ada versi GLIBC di atas `2.17` pada output tersebut, binary dipastikan kompatibel untuk berjalan di distribusi Linux lama maupun baru.
