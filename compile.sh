#!/bin/bash

# ==============================================================================
# 🛠️ ZenoPanel & zeno-container Interactive Compiler & Packager (v2)
# ==============================================================================
# Script ini digunakan untuk mengompilasi ZenoPanel dan runtime zeno-container,
# memverifikasi binary, serta mengemas seluruh aset ke folder /dist.
# ==============================================================================

# Definisikan warna untuk visualisasi
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Helper logging
log_info() {
    echo -e "${BLUE}[i]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[!]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

# Tampilkan Banner
echo -e "${CYAN}${BOLD}"
echo "=================================================="
echo "    ZenoPanel Compilation & Packaging Tools       "
echo "=================================================="
echo -e "${NC}"

# ------------------------------------------------------------------------------
# 1. Parsing Argumen CLI
# ------------------------------------------------------------------------------
NON_INTERACTIVE=false
CLEAN_BUILD=false
TARGET_CHOICE=""
INPUT_VERSION=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --non-interactive|-y)
            NON_INTERACTIVE=true
            shift
            ;;
        --target)
            TARGET_CHOICE="$2"
            shift 2
            ;;
        --version)
            INPUT_VERSION="$2"
            shift 2
            ;;
        --clean)
            CLEAN_BUILD=true
            shift
            ;;
        --help|-h)
            echo "Penggunaan: $0 [Opsi]"
            echo "Opsi:"
            echo "  --non-interactive, -y    Jalankan tanpa prompt interaktif (menggunakan default/argumen)"
            echo "  --target [musl|gnu]      Pilih target kompilasi (default: musl)"
            echo "  --version [versi]        Tentukan versi paket distribusi (default: dari git tag/Cargo.toml)"
            echo "  --clean                  Lakukan pembersihan cache kompilasi sebelum build"
            echo "  --help, -h               Tampilkan bantuan ini"
            exit 0
            ;;
        *)
            log_error "Opsi tidak dikenal: $1. Gunakan --help untuk melihat bantuan."
            exit 1
            ;;
    esac
done

# ------------------------------------------------------------------------------
# 2. Pendeteksian Versi Otomatis (Git Tag / Cargo.toml)
# ------------------------------------------------------------------------------
# Mencoba mendeteksi tag git terakhir
GIT_TAG=$(git describe --tags --abbrev=0 2>/dev/null)

if [ -n "$GIT_TAG" ]; then
    DEFAULT_VERSION="$GIT_TAG"
    log_info "Mendeteksi tag git terakhir: ${BOLD}$DEFAULT_VERSION${NC}"
else
    # Fallback ke Cargo.toml jika tidak ada tag git
    CARGO_VERSION=$(grep '^version =' Cargo.toml | head -n1 | cut -d '"' -f2 2>/dev/null)
    if [ -z "$CARGO_VERSION" ]; then
        CARGO_VERSION="0.1.0"
    fi
    DEFAULT_VERSION="v${CARGO_VERSION}"
    log_info "Tidak ada tag git ditemukan. Menggunakan versi Cargo.toml: ${BOLD}$DEFAULT_VERSION${NC}"
fi

# ------------------------------------------------------------------------------
# 3. Penentuan Target Kompilasi
# ------------------------------------------------------------------------------
if [ "$NON_INTERACTIVE" = false ]; then
    echo -e "\nPilih target kompilasi ZenoPanel:"
    echo -e "  1) ${BOLD}x86_64-unknown-linux-musl${NC} (Static binary murni - Default)"
    echo -e "  2) ${BOLD}x86_64-unknown-linux-gnu${NC} (Kompatibilitas GLIBC 2.17+)"
    echo -n "Masukkan pilihan Anda (1/2, default 1): "
    read -r TARGET_INPUT
    if [ -n "$TARGET_INPUT" ]; then
        TARGET_CHOICE="$TARGET_INPUT"
    fi
fi

if [ "$TARGET_CHOICE" = "2" ] || [ "$TARGET_CHOICE" = "gnu" ]; then
    TARGET="x86_64-unknown-linux-gnu"
    ZIG_TARGET="x86_64-linux-gnu.2.17"
else
    TARGET="x86_64-unknown-linux-musl"
    ZIG_TARGET="x86_64-linux-musl"
fi
log_info "Target terpilih: ${BOLD}$TARGET${NC} ($ZIG_TARGET)"

# ------------------------------------------------------------------------------
# 4. Penentuan Versi Paket
# ------------------------------------------------------------------------------
if [ "$NON_INTERACTIVE" = false ]; then
    echo -n "Masukkan versi paket distribusi (default ${DEFAULT_VERSION}): "
    read -r VERSION_INPUT
    if [ -n "$VERSION_INPUT" ]; then
        INPUT_VERSION="$VERSION_INPUT"
    fi
fi

if [ -z "$INPUT_VERSION" ]; then
    PKG_VERSION="$DEFAULT_VERSION"
else
    PKG_VERSION="$INPUT_VERSION"
fi
log_info "Versi paket distribusi: ${BOLD}$PKG_VERSION${NC}"

# Sinkronisasi versi ke file konfigurasi dan source code
CLEAN_VERSION=$(echo "$PKG_VERSION" | sed 's/^v//')
V_VERSION="v$CLEAN_VERSION"
log_info "Menyelaraskan versi ZenoPanel ke ${BOLD}${V_VERSION}${NC}..."

if [ -f "Cargo.toml" ]; then
    sed -i 's/^version = "[^"]*"/version = "'"$CLEAN_VERSION"'"/' Cargo.toml
    log_success "Versi di Cargo.toml diperbarui menjadi: $CLEAN_VERSION"
fi

if [ -f "install.sh" ]; then
    sed -i 's/^DEFAULT_VERSION="[^"]*"/DEFAULT_VERSION="'"$V_VERSION"'"/' install.sh
    log_success "Versi default di install.sh diperbarui menjadi: $V_VERSION"
fi

if [ -f "views/partials/sidebar.blade.zl" ]; then
    sed -i 's/ZenoPanel v[0-9a-zA-Z.-]*/ZenoPanel '"$V_VERSION"'/' views/partials/sidebar.blade.zl
    log_success "Versi di views/partials/sidebar.blade.zl diperbarui menjadi: $V_VERSION"
fi

# ------------------------------------------------------------------------------
# 5. Pembersihan Cache Kompilasi (Clean Build)
# ------------------------------------------------------------------------------
if [ "$NON_INTERACTIVE" = false ] && [ "$CLEAN_BUILD" = false ]; then
    echo -n "Lakukan pembersihan cache kompilasi sebelum build? (y/N, default N): "
    read -r CLEAN_INPUT
    if [[ "$CLEAN_INPUT" =~ ^[yY]$ ]]; then
        CLEAN_BUILD=true
    fi
fi

if [ "$CLEAN_BUILD" = true ]; then
    log_info "Membersihkan cache kompilasi..."
    cargo clean
    log_success "Pembersihan cache kompilasi berhasil."
fi

# ------------------------------------------------------------------------------
# 7. Verifikasi Lingkungan & Dependensi
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}Memverifikasi dependensi sistem...${NC}"

# Cek Cargo
if ! command -v cargo >/dev/null 2>&1; then
    log_error "Rust/Cargo tidak ditemukan. Silakan pasang Rust terlebih dahulu."
    exit 1
fi
log_success "Rust/Cargo terpasang."

# Cek Zig (untuk cross compilation)
if ! command -v zig >/dev/null 2>&1; then
    log_error "Zig compiler tidak ditemukan. Zig diperlukan sebagai linker untuk target glibc lama maupun musl."
    log_error "Pasang zig melalui package manager Anda (misal: 'sudo apt install zig' atau 'snap install zig --classic')."
    exit 1
fi
log_success "Zig compiler terpasang."

# Cek Target Rust
if ! rustup target list --installed | grep -q "$TARGET"; then
    log_warn "Target Rust '$TARGET' belum terpasang."
    if [ "$NON_INTERACTIVE" = true ]; then
        log_info "Memasang target Rust '$TARGET' secara otomatis di mode non-interaktif..."
        rustup target add "$TARGET" || { log_error "Gagal memasang target"; exit 1; }
    else
        echo -n "Pasang target '$TARGET' secara otomatis? (Y/n, default Y): "
        read -r INSTALL_TARGET
        if [[ ! "$INSTALL_TARGET" =~ ^[nN]$ ]]; then
            log_info "Memasang target Rust '$TARGET' via rustup..."
            rustup target add "$TARGET"
            if [ $? -ne 0 ]; then
                log_error "Gagal memasang target '$TARGET'."
                exit 1
            fi
            log_success "Target '$TARGET' berhasil terpasang."
        else
            log_error "Target '$TARGET' diperlukan untuk kompilasi."
            exit 1
        fi
    fi
else
    log_success "Target Rust '$TARGET' sudah siap."
fi

# Set permission wrapper script
chmod +x zig_wrappers/*.sh 2>/dev/null
log_success "Izin eksekusi wrapper zig_wrappers/*.sh telah disesuaikan."

# Inject local cmake jika tersedia
if [ -d "$PWD/cmake_local/bin" ]; then
    export PATH="$PWD/cmake_local/bin:$PATH"
    log_info "Menyuntikkan cmake_local/bin ke PATH."
fi

# ------------------------------------------------------------------------------
# 8. Proses Kompilasi
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}Memulai proses kompilasi...${NC}"

# Kompilasi Rust (ZenoPanel)
log_info "Mengompilasi ZenoPanel untuk target $TARGET..."
if [ "$TARGET" = "x86_64-unknown-linux-musl" ]; then
    RUSTFLAGS="-C link-self-contained=no" \
    ZIG_TARGET="x86_64-linux-musl" \
    CC_x86_64_unknown_linux_musl="$PWD/zig_wrappers/zig-cc.sh" \
    CXX_x86_64_unknown_linux_musl="$PWD/zig_wrappers/zig-cxx.sh" \
    AR_x86_64_unknown_linux_musl="$PWD/zig_wrappers/zig-ar.sh" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$PWD/zig_wrappers/zig-linker-wrapper.sh" \
    cargo build --release --target x86_64-unknown-linux-musl
else
    ZIG_TARGET="x86_64-linux-gnu.2.17" \
    CC_x86_64_unknown_linux_gnu="$PWD/zig_wrappers/zig-cc.sh" \
    CXX_x86_64_unknown_linux_gnu="$PWD/zig_wrappers/zig-cxx.sh" \
    AR_x86_64_unknown_linux_gnu="$PWD/zig_wrappers/zig-ar.sh" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$PWD/zig_wrappers/zig-linker-wrapper.sh" \
    cargo build --release --target x86_64-unknown-linux-gnu
fi

if [ $? -ne 0 ]; then
    log_error "Kompilasi ZenoPanel gagal!"
    exit 1
fi
log_success "Kompilasi ZenoPanel berhasil."

# ------------------------------------------------------------------------------
# 9. Pengemasan ke Folder /dist
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}Memulai pengemasan paket distribusi...${NC}"

DIST_DIR="dist"
PKG_NAME="zenopanel-${PKG_VERSION}"
PKG_PATH="${DIST_DIR}/${PKG_NAME}"

# Bersihkan berkas distribusi lama (*.tar.gz dan *.tar.gz.sha256)
log_info "Membersihkan berkas distribusi lama di folder ${DIST_DIR}..."
rm -f "${DIST_DIR}"/zenopanel-*.tar.gz "${DIST_DIR}"/zenopanel-*.tar.gz.sha256 2>/dev/null

# Buat folder target
mkdir -p "$PKG_PATH"
log_info "Folder tujuan pengemasan siap: $PKG_PATH"

# Salin aset
log_info "Menyalin berkas-berkas aset pendukung..."
cp -r public "$PKG_PATH/"
cp -r views "$PKG_PATH/"
cp -r zsrc "$PKG_PATH/"
cp .env.example "$PKG_PATH/"

# Salin binary utama
log_info "Menyalin binary zeno..."
cp "target/${TARGET}/release/zeno" "$PKG_PATH/"

# Optimasi ukuran binary dengan strip
log_info "Mengoptimalkan ukuran binary dengan memotong symbol debug (strip)..."
strip "$PKG_PATH/zeno" 2>/dev/null

# Verifikasi hasil akhir binary
log_info "Memverifikasi tipe binary..."
if command -v file >/dev/null 2>&1; then
    echo -e "${CYAN}${BOLD}[ZenoPanel Binary Info]${NC}"
    file "$PKG_PATH/zeno"
else
    log_warn "Utilitas 'file' tidak terpasang di sistem. Melewati verifikasi."
fi

# Kompresi folder ke tar.gz
log_info "Mengompresi paket distribusi menjadi tarball..."
cd "$DIST_DIR" || exit 1
tar -czf "${PKG_NAME}.tar.gz" "${PKG_NAME}"

# Pembuatan berkas checksum SHA-256
log_info "Membuat berkas checksum SHA-256..."
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256"
    log_success "Berkas checksum SHA-256 dibuat."
else
    log_warn "Utilitas 'sha256sum' tidak ditemukan, mencoba menggunakan 'shasum'..."
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256"
        log_success "Berkas checksum SHA-256 dibuat."
    else
        log_warn "Utilitas sha256sum/shasum tidak ditemukan. Pembuatan checksum dilewati."
    fi
fi
cd - > /dev/null || exit 1

# Hapus folder temporer
log_info "Membersihkan direktori temporer..."
rm -rf "$PKG_PATH"

log_success "Selamat! Pengemasan berhasil selesai!"
echo -e "\n${BOLD}Detail Hasil Akhir:${NC}"
echo -e "  - Berkas Output   : ${GREEN}${PWD}/${DIST_DIR}/${PKG_NAME}.tar.gz${NC}"
echo -e "  - Berkas Checksum : ${GREEN}${PWD}/${DIST_DIR}/${PKG_NAME}.tar.gz.sha256${NC}"
echo -e "  - Ukuran Berkas   : ${GREEN}$(du -sh "${DIST_DIR}/${PKG_NAME}.tar.gz" | cut -f1)${NC}"
echo -e "=================================================="
