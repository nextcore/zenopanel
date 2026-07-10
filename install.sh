#!/bin/bash

# ==============================================================================
# 🚀 ZenoPanel Automated Installer Script
# ==============================================================================
# Script ini digunakan untuk mengunduh, memverifikasi, mengekstrak, dan
# mempersiapkan lingkungan ZenoPanel secara otomatis di server target.
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
echo "          ZenoPanel Installer System              "
echo "=================================================="
echo -e "${NC}"

# ------------------------------------------------------------------------------
# 1. Verifikasi Kompatibilitas Sistem
# ------------------------------------------------------------------------------
if [ "$(uname -s)" != "Linux" ]; then
    log_error "ZenoPanel hanya mendukung sistem operasi Linux saat ini."
    exit 1
fi

if [ "$(uname -m)" != "x86_64" ]; then
    log_error "Arsitektur mesin $(uname -m) tidak didukung. Hanya mendukung x86_64."
    exit 1
fi

# ------------------------------------------------------------------------------
# 2. Definisikan Versi Default & Direktori Tujuan
# ------------------------------------------------------------------------------
DEFAULT_VERSION="v1.5.0"
DEFAULT_INSTALL_DIR="/opt/zenopanel"

# Baca parameter argumen jika ada
VERSION="$DEFAULT_VERSION"
INSTALL_DIR="$DEFAULT_INSTALL_DIR"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        *)
            log_error "Opsi tidak dikenal: $1"
            echo "Penggunaan: $0 [--version <versi>] [--dir <direktori>]"
            exit 1
            ;;
    esac
done

# Jika mode interaktif, tanyakan kepada user untuk konfirmasi
if [ -t 0 ]; then
    echo -n "Tentukan direktori instalasi (default ${DEFAULT_INSTALL_DIR}): "
    read -r DIR_INPUT
    if [ -n "$DIR_INPUT" ]; then
        INSTALL_DIR="$DIR_INPUT"
    fi

    echo -n "Tentukan versi ZenoPanel yang ingin dipasang (default ${VERSION}): "
    read -r VER_INPUT
    if [ -n "$VER_INPUT" ]; then
        VERSION="$VER_INPUT"
    fi
fi

log_info "Lokasi instalasi : ${BOLD}${INSTALL_DIR}${NC}"
log_info "Versi target      : ${BOLD}${VERSION}${NC}"

# Buat direktori instalasi jika belum ada
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR" || { log_error "Gagal masuk ke direktori $INSTALL_DIR"; exit 1; }

# ------------------------------------------------------------------------------
# 3. Proses Pengunduhan Paket & Checksum
# ------------------------------------------------------------------------------
REPO_URL="https://github.com/nextcore/zenopanel/releases/download/${VERSION}"
TARBALL_FILE="zenopanel-${VERSION}.tar.gz"
CHECKSUM_FILE="zenopanel-${VERSION}.tar.gz.sha256"

log_info "Mengunduh paket rilis dan berkas checksum..."

# Hapus berkas lama jika ada
rm -f "$TARBALL_FILE" "$CHECKSUM_FILE"

# Fungsi unduh dengan validasi HTTP status code dan retry pada 429
download_file() {
    local url="$1"
    local output="$2"
    local max_retry=3
    local attempt=1

    while [ $attempt -le $max_retry ]; do
        local http_code
        if command -v curl >/dev/null 2>&1; then
            http_code=$(curl -L -w "%{http_code}" -o "$output" -# "$url" 2>/dev/null)
        elif command -v wget >/dev/null 2>&1; then
            wget -q --server-response -O "$output" "$url" 2>/tmp/zeno_wget_header
            http_code=$(grep "HTTP/" /tmp/zeno_wget_header | tail -1 | awk '{print $2}')
            rm -f /tmp/zeno_wget_header
        else
            log_error "Wget atau Curl diperlukan untuk mengunduh berkas. Silakan pasang salah satunya."
            exit 1
        fi

        if [ "$http_code" = "200" ]; then
            return 0
        elif [ "$http_code" = "429" ]; then
            log_warn "Server membatasi kecepatan unduhan (429). Menunggu 5 detik... (percobaan ${attempt}/${max_retry})"
            rm -f "$output"
            sleep 5
            attempt=$((attempt + 1))
        elif [ "$http_code" = "404" ]; then
            log_error "Berkas tidak ditemukan di repositori (404): $(basename "$url")"
            log_error "Pastikan versi ${VERSION} sudah tersedia di repositori."
            rm -f "$output"
            exit 1
        else
            log_error "Gagal mengunduh $(basename "$url") (HTTP ${http_code})."
            rm -f "$output"
            exit 1
        fi
    done

    log_error "Gagal mengunduh setelah ${max_retry} percobaan. Server membatasi akses (429)."
    log_error "Coba jalankan ulang installer beberapa saat lagi."
    exit 1
}

log_info "Mengunduh tarball distribusi..."
download_file "${REPO_URL}/${TARBALL_FILE}" "$TARBALL_FILE"
log_success "Tarball berhasil diunduh."

log_info "Mengunduh berkas checksum..."
download_file "${REPO_URL}/${CHECKSUM_FILE}" "$CHECKSUM_FILE"
log_success "Checksum berhasil diunduh."

if [ ! -f "$TARBALL_FILE" ] || [ ! -f "$CHECKSUM_FILE" ]; then
    log_error "Gagal mengunduh berkas instalasi untuk versi ${VERSION}."
    log_error "Pastikan versi tersebut sudah tersedia di repositori."
    exit 1
fi

# ------------------------------------------------------------------------------
# 4. Verifikasi Integritas Tarball
# ------------------------------------------------------------------------------
log_info "Memverifikasi integritas berkas..."

# Patch path checksum jika ada perbedaan format direktori
sed -i "s|dist/||g" "$CHECKSUM_FILE" 2>/dev/null

VERIFIED=false
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$CHECKSUM_FILE" >/dev/null 2>&1 && VERIFIED=true
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$CHECKSUM_FILE" >/dev/null 2>&1 && VERIFIED=true
fi

if [ "$VERIFIED" = false ]; then
    log_error "Verifikasi checksum berkas gagal! Berkas tarball korup atau telah dimodifikasi."
    exit 1
fi
log_success "Verifikasi integritas berkas sukses."

# ------------------------------------------------------------------------------
# 5. Ekstraksi Berkas
# ------------------------------------------------------------------------------
log_info "Mengekstrak berkas distribusi..."

# Gunakan direktori sementara untuk mengekstrak tanpa duplikasi nama subfolder
TEMP_EXTRACT_DIR="zeno_temp_extract"
mkdir -p "$TEMP_EXTRACT_DIR"
tar -xzf "$TARBALL_FILE" -C "$TEMP_EXTRACT_DIR"

# Pindahkan isi dari folder hasil ekstrak ke direktori target utama
EXTRACTED_SUBFOLDER=$(ls "$TEMP_EXTRACT_DIR")
if [ -n "$EXTRACTED_SUBFOLDER" ]; then
    cp -r "$TEMP_EXTRACT_DIR/$EXTRACTED_SUBFOLDER"/* ./
    cp -r "$TEMP_EXTRACT_DIR/$EXTRACTED_SUBFOLDER"/.env.example ./ 2>/dev/null
fi

# Bersihkan sisa ekstraksi dan berkas tarball
rm -rf "$TEMP_EXTRACT_DIR" "$TARBALL_FILE" "$CHECKSUM_FILE"

# ------------------------------------------------------------------------------
# 6. Inisialisasi Konfigurasi Environment & Token Keamanan
# ------------------------------------------------------------------------------
if [ ! -f ".env" ]; then
    log_info "Membuat berkas konfigurasi .env..."
    if [ -f ".env.example" ]; then
        cp .env.example .env
    else
        log_warn ".env.example tidak ditemukan. Membuat .env kosong."
        touch .env
    fi
else
    log_info "Berkas .env sudah ada, melewati pembuatan baru."
fi

# Generate JWT_SECRET otomatis menggunakan binary zeno jika tersedia
if [ -f "./zeno" ]; then
    chmod +x zeno 2>/dev/null
    log_info "Men-generate JWT_SECRET otomatis untuk keamanan..."
    ./zeno key:generate >/dev/null 2>&1
    if [ $? -eq 0 ]; then
        log_success "JWT_SECRET berhasil diperbarui di .env."
    else
        log_warn "Gagal men-generate JWT_SECRET secara otomatis via binary."
    fi
fi

# ------------------------------------------------------------------------------
# 7. Selesai
# ------------------------------------------------------------------------------
log_success "Instalasi ZenoPanel sukses dilakukan!"
echo -e "\n${BOLD}Langkah Selanjutnya:${NC}"
echo -e "  1. Masuk ke direktori instalasi:"
echo -e "     ${CYAN}cd ${INSTALL_DIR}${NC}"
echo -e "  2. Sesuaikan konfigurasi di berkas ${BOLD}.env${NC} jika diperlukan."
echo -e "  3. Jalankan aplikasi ZenoPanel:"
echo -e "     ${CYAN}./zeno${NC}"
echo -e "  4. Pasang sebagai systemd service melalui antarmuka web ZenoPanel:"
echo -e "     Menu ${BOLD}Settings${NC} -> ${BOLD}Service Injector${NC} -> Pilih ${BOLD}Install${NC}."
echo -e "=================================================="
