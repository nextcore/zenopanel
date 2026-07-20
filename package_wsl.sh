#!/bin/bash

# ==============================================================================
# 📦 ZenoPanel WSL2 Distro Packager (Alpine-based)
# ==============================================================================
# Skrip ini mengemas paket distribusi ZenoPanel yang ada di /dist ke dalam
# distro WSL2 kustom berbasis Alpine Linux terbaru.
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
echo "      ZenoPanel WSL2 Distro Packaging Tool        "
echo "=================================================="
echo -e "${NC}"

# 1. Deteksi Berkas ZenoPanel Kompilasi di Folder dist/ & Versi
log_info "Mencari berkas ZenoPanel terkompilasi di folder dist/..."

# Cari berkas dist/zenopanel-*.tar.gz (abaikan dist/zenopanel-windows-*.zip)
LOCAL_ZENOPANEL_TAR=$(ls -t dist/zenopanel-v*.tar.gz 2>/dev/null | head -n1)

if [ -z "$LOCAL_ZENOPANEL_TAR" ]; then
    LOCAL_ZENOPANEL_TAR=$(ls -t dist/zenopanel-*.tar.gz 2>/dev/null | grep -v 'zenopanel-windows' | grep -v 'zenoos' | head -n1)
fi

if [ -n "$LOCAL_ZENOPANEL_TAR" ] && [ -f "$LOCAL_ZENOPANEL_TAR" ]; then
    log_success "Ditemukan berkas terkompilasi lokal: ${BOLD}$LOCAL_ZENOPANEL_TAR${NC}"
    FILENAME=$(basename "$LOCAL_ZENOPANEL_TAR")
    VERSION=$(echo "$FILENAME" | sed -E 's/^zenopanel-(.*)\.tar\.gz$/\1/')
    if [[ "$VERSION" != v* ]]; then
        VERSION="v${VERSION}"
    fi
else
    log_warn "Berkas ZenoPanel terkompilasi (dist/zenopanel-*.tar.gz) tidak ditemukan di dist/."
    log_info "Mendeteksi versi dari Cargo.toml untuk mengompilasi otomatis..."
    CARGO_VERSION=$(grep '^version =' Cargo.toml | head -n1 | cut -d '"' -f2 2>/dev/null)
    VERSION="v${CARGO_VERSION:-1.5.22}"

    log_info "Menjalankan ./compile.sh untuk mengompilasi ZenoPanel Linux Standalone..."
    ./compile.sh --target musl --non-interactive
    if [ $? -ne 0 ]; then
        log_error "Kompilasi ZenoPanel gagal!"
        exit 1
    fi
    LOCAL_ZENOPANEL_TAR="dist/zenopanel-${VERSION}.tar.gz"
fi

log_success "Versi target ZenoPanel: ${BOLD}$VERSION${NC}"

# 2. Cari & Unduh Alpine Minrootfs Terbaru
log_info "Mencari versi Alpine Linux stabil terbaru..."
INDEX_HTML=$(curl -sL https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/)
ALPINE_FILE=$(echo "$INDEX_HTML" | grep -oE 'alpine-minirootfs-[0-9]+\.[0-9]+\.[0-9]+-x86_64\.tar\.gz' | head -n1)

if [ -z "$ALPINE_FILE" ]; then
    log_warn "Gagal mendeteksi rilis otomatis di CDN. Menggunakan fallback versi Alpine 3.24.1..."
    ALPINE_FILE="alpine-minirootfs-3.24.1-x86_64.tar.gz"
fi

ALPIN_URL="https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/$ALPINE_FILE"
log_success "Menggunakan Alpine minrootfs: ${BOLD}$ALPINE_FILE${NC}"
ALPINE_VERSION=$(echo "$ALPINE_FILE" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')

# Unduh Alpine Minrootfs dengan Caching
CACHE_DIR="dist/cache"
mkdir -p "$CACHE_DIR"

if [ ! -f "$CACHE_DIR/$ALPINE_FILE" ]; then
    log_info "Mengunduh Alpine minrootfs dari CDN..."
    curl -L -# -o "$CACHE_DIR/$ALPINE_FILE" "$ALPIN_URL"
    if [ $? -ne 0 ]; then
        log_error "Gagal mengunduh Alpine minrootfs."
        exit 1
    fi
    log_success "Unduhan selesai."
else
    log_info "Menggunakan berkas Alpine minrootfs dari cache lokal."
fi

# 3. Persiapkan Lingkungan Pembuatan Rootfs
BUILD_DIR="dist/wsl_build"

log_info "Membersihkan sisa-sisa build lama..."
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

# 4. Ekstrak Alpine Minrootfs
log_info "Mengekstrak Alpine minrootfs..."
tar -xzf "$CACHE_DIR/$ALPINE_FILE" -C "$BUILD_DIR"
if [ $? -ne 0 ]; then
    log_error "Gagal mengekstrak Alpine minrootfs."
    exit 1
fi
log_success "Ekstraksi Alpine berhasil."

# 5. Ekstrak ZenoPanel yang sudah dicompilasi ke dalam distro
log_info "Memasukkan paket ZenoPanel ($LOCAL_ZENOPANEL_TAR) ke dalam distro..."
mkdir -p "$BUILD_DIR/opt/zenopanel"
tar -xzf "$LOCAL_ZENOPANEL_TAR" -C "$BUILD_DIR/opt"
EXTRACTED_DIR=$(find "$BUILD_DIR/opt" -maxdepth 1 -mindepth 1 -type d -name "zenopanel-*" | head -n1)
if [ -n "$EXTRACTED_DIR" ] && [ -d "$EXTRACTED_DIR" ]; then
    cp -r "$EXTRACTED_DIR"/* "$BUILD_DIR/opt/zenopanel/"
    cp -r "$EXTRACTED_DIR"/.env.example "$BUILD_DIR/opt/zenopanel/" 2>/dev/null
    rm -rf "$EXTRACTED_DIR"
fi

# Inisialisasi berkas .env bawaan untuk lingkungan WSL2
if [ -f "$BUILD_DIR/opt/zenopanel/.env.example" ]; then
    cp "$BUILD_DIR/opt/zenopanel/.env.example" "$BUILD_DIR/opt/zenopanel/.env"
    sed -i 's/^APP_PORT=.*/APP_PORT=:3001/' "$BUILD_DIR/opt/zenopanel/.env"
    sed -i 's/^APP_TLS_PORT=.*/APP_TLS_PORT=:8443/' "$BUILD_DIR/opt/zenopanel/.env"
    sed -i 's/^MGMT_PORT=.*/MGMT_PORT=:3002/' "$BUILD_DIR/opt/zenopanel/.env"
fi

# 6. Buat Direktori Data Tambahan & Rebranding ZenoOS
log_info "Mempersiapkan direktori tambahan & melakukan rebranding ke ZenoOS..."
mkdir -p "$BUILD_DIR/var/lib/zeno-container"
mkdir -p "$BUILD_DIR/run/openrc"

# Rebranding /etc/os-release
cat << EOF > "$BUILD_DIR/etc/os-release"
NAME="ZenoOS"
VERSION="${VERSION} (Based on Alpine v${ALPINE_VERSION})"
ID=zenoos
ID_LIKE=alpine
PRETTY_NAME="ZenoOS ${VERSION}"
LOGO=zenopanel-logo
HOME_URL="https://github.com/nextcore/zenopanel"
SUPPORT_URL="https://github.com/nextcore/zenopanel/issues"
BUG_REPORT_URL="https://github.com/nextcore/zenopanel/issues"
EOF

# Rebranding /etc/issue & /etc/motd
cat << EOF > "$BUILD_DIR/etc/issue"
Welcome to ZenoOS ${VERSION} (Based on Alpine Linux v${ALPINE_VERSION})
EOF

cat << EOF > "$BUILD_DIR/etc/motd"
  ⚡ ZenoOS ${VERSION} (WSL2 Distro)
  ===========================================
  ZenoPanel Server Control Center is running!
  
  🌐 Access Dashboard: http://localhost:3001/login
  📂 Data Directory: /var/lib/zeno-container
  🔧 Mode: WSL2 Optimized (Alpine v${ALPINE_VERSION} Base)
  ===========================================
EOF

# Tulis file /etc/wsl.conf bawaan untuk tuning WSL2
cat << EOF > "$BUILD_DIR/etc/wsl.conf"
[boot]
systemd=false

[network]
generateResolvConf=true

[automount]
enabled=true
options="metadata,uid=1000,gid=1000,umask=022"
EOF

# 7. Tambahkan Helper/Launcher script di /usr/local/bin
log_info "Membuat skrip pembantu (launcher) di /usr/local/bin/zenopanel..."
cat << EOF > "$BUILD_DIR/usr/local/bin/zenopanel"
#!/bin/sh
# Launcher ZenoPanel untuk WSL2

# Sinkronisasi jam sistem dengan hardware host Windows untuk mencegah SSL error akibat clock drift
echo "[ZenoOS] Sinkronisasi waktu sistem..."
hwclock -s >/dev/null 2>&1

# Cek & pasang dependensi dasar (iptables, iproute2) jika belum ada
if ! command -v iptables >/dev/null 2>&1 || ! command -v ip >/dev/null 2>&1; then
    echo "[ZenoOS] Menyiapkan paket-paket dasar (iptables, iproute2)..."
    apk update >/dev/null 2>&1
    apk add --no-cache iptables iproute2 >/dev/null 2>&1
fi

cd /opt/zenopanel || exit 1

# Generate JWT_SECRET otomatis jika belum ada di .env
if [ -f .env ] && ! grep -q "^JWT_SECRET=" .env; then
    echo "[ZenoPanel] Menginisialisasi JWT_SECRET baru di .env..."
    ./zeno key:generate >/dev/null 2>&1
fi

exec ./zeno "\$@"
EOF

chmod +x "$BUILD_DIR/usr/local/bin/zenopanel"
ln -sf /opt/zenopanel/zeno "$BUILD_DIR/usr/local/bin/zeno"

# 8. Kemas Ulang sebagai Distro ZenoOS WSL2
TAR_NAME="zenoos-${VERSION}"
OUTPUT_FILE="dist/${TAR_NAME}.tar.gz"

log_info "Mengompresi distro WSL2 kustom..."
rm -f "$OUTPUT_FILE" "${OUTPUT_FILE}.sha256"

# Buat tar.gz
cd "$BUILD_DIR" || exit 1
tar -czf "../${TAR_NAME}.tar.gz" .
cd - > /dev/null || exit 1

# 10. Kompilasi Windows Launcher (.exe) berbasis Zig
LAUNCHER_TEMP="dist/zenopanel-launcher.exe"
log_info "Mengompilasi Windows Launcher (.exe) berbasis Zig..."

# Update version in main.zig and zenopanel.ps1
sed -i 's|const VERSION = .*|const VERSION = "'"${VERSION}"'";|' launcher/main.zig
sed -i 's|^\$VERSION = .*|\$VERSION = "'"${VERSION}"'"|' launcher/zenopanel.ps1
sed -i 's|PowerShell GUI Edition (v[0-9a-zA-Z.-]*)|PowerShell GUI Edition ('"${VERSION}"')|' launcher/zenopanel.ps1

zig build-exe -target x86_64-windows-gnu -O ReleaseSmall -femit-bin="$LAUNCHER_TEMP" launcher/main.zig
if [ $? -ne 0 ]; then
    log_error "Kompilasi Windows Launcher .exe gagal!"
    exit 1
fi
log_success "Windows Launcher .exe berhasil dikompilasi."

# 11. Kemas SATU berkas ZIP rilis final (launcher + ps1 + zenoos distro)
ZIP_NAME="zenopanel-windows-${VERSION}"
ZIP_FILE="dist/${ZIP_NAME}.zip"

if command -v zip > /dev/null 2>&1; then
    log_info "Mengemas ke satu berkas ZIP distribusi final..."
    rm -f "$ZIP_FILE"
    zip -j "$ZIP_FILE" "$LAUNCHER_TEMP" "launcher/zenopanel.ps1" "$OUTPUT_FILE" > /dev/null
    if [ $? -eq 0 ]; then
        log_success "Berkas ZIP final berhasil dibuat: ${ZIP_FILE}"
    else
        log_error "Gagal membuat berkas ZIP."
        exit 1
    fi
else
    log_error "Utilitas 'zip' tidak ditemukan. Pasang zip lalu jalankan ulang."
    exit 1
fi

# Buat checksum untuk ZIP (bukan tarball terpisah)
if command -v sha256sum > /dev/null 2>&1; then
    sha256sum "$ZIP_FILE" > "${ZIP_FILE}.sha256"
    log_success "Checksum SHA-256 ZIP dibuat."
fi

# 12. Bersihkan berkas sementara (tarball zenoos & launcher standalone)
log_info "Membersihkan berkas sementara..."
rm -f "$OUTPUT_FILE" "$LAUNCHER_TEMP"
# Hapus juga artefak build tambahan Zig
rm -f dist/zenopanel-launcher.exe.obj 2>/dev/null

# 13. Pembersihan Folder Temporer
log_info "Membersihkan direktori kerja..."
rm -rf "$BUILD_DIR"

# Selesai
log_success "Selamat! Pengemasan distro WSL2 berhasil diselesaikan!"
echo -e "\n${BOLD}Detail Hasil Akhir:${NC}"
echo -e "  - Berkas Rilis : ${GREEN}${PWD}/${ZIP_FILE}${NC}"
echo -e "                   ${CYAN}(launcher.exe + zenopanel.ps1 + ZenoOS distro — siap unggah ke GitHub Releases & bagikan ke pengguna)${NC}"
echo -e "  - Checksum     : ${GREEN}${PWD}/${ZIP_FILE}.sha256${NC}"
echo -e "  - Ukuran       : ${GREEN}$(du -sh "${ZIP_FILE}" | cut -f1)${NC}"
echo -e "=================================================="
echo -e "\n${BOLD}Langkah Rilis:${NC}"
echo -e "  1. Unggah ${GREEN}$(basename "$ZIP_FILE")${NC} ke GitHub Releases dengan tag ${YELLOW}${VERSION}${NC}."
echo -e "  2. Bagikan tautan download kepada pengguna Windows."
echo -e "  3. Pengguna ekstrak ZIP, jalankan ${GREEN}zenopanel-launcher.exe${NC} atau ${GREEN}zenopanel.ps1${NC}."
echo -e "=================================================="
