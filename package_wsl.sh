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

# 1. Deteksi Versi ZenoPanel dari Git / Cargo.toml
log_info "Mendeteksi versi ZenoPanel..."
GIT_TAG=$(git describe --tags --abbrev=0 2>/dev/null)
if [ -n "$GIT_TAG" ]; then
    VERSION="$GIT_TAG"
else
    CARGO_VERSION=$(grep '^version =' Cargo.toml | head -n1 | cut -d '"' -f2 2>/dev/null)
    VERSION="v${CARGO_VERSION:-1.5.19}"
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

# 5. Buat direktori ZenoPanel kosong
mkdir -p "$BUILD_DIR/opt/zenopanel"

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

# Uji koneksi DNS ke github. Jika gagal, gunakan DNS Fallback publik (Cloudflare/Google)
# agar install.sh tidak gagal akibat isu DNS di WSL2/VPN
if ! ping -c 1 -W 2 raw.githubusercontent.com >/dev/null 2>&1; then
    echo "[ZenoOS] Koneksi internet/DNS terhambat. Mengaktifkan DNS Fallback (1.1.1.1)..."
    rm -f /etc/resolv.conf
    echo -e "nameserver 1.1.1.1\nnameserver 8.8.8.8" > /etc/resolv.conf
fi

# Cek & pasang dependensi dasar (curl, ca-certificates, tar, gzip, iptables, iproute2) jika belum ada
if ! command -v curl >/dev/null 2>&1 || ! command -v tar >/dev/null 2>&1 || ! command -v iptables >/dev/null 2>&1 || ! command -v ip >/dev/null 2>&1; then
    echo "[ZenoOS] Menyiapkan paket-paket dasar (curl, ca-certificates, tar, gzip, iptables, iproute2)..."
    apk update >/dev/null 2>&1
    apk add --no-cache curl ca-certificates tar gzip iptables iproute2 >/dev/null 2>&1
fi

# Cek apakah ZenoPanel sudah terpasang
if [ ! -f /opt/zenopanel/zeno ]; then
    echo "[ZenoOS] Mengunduh dan memasang ZenoPanel ${VERSION} otomatis..."
    # Jalankan install.sh secara senyap (tanpa terminal interactive) untuk memasang versi yang ditargetkan
    curl -sL https://raw.githubusercontent.com/nextcore/zenopanel/main/install.sh | sh -s -- --version "${VERSION}" --dir /opt/zenopanel < /dev/null
    
    # Sesuaikan port setelah terpasang untuk lingkungan Windows/WSL2
    if [ -f /opt/zenopanel/.env ]; then
        sed -i 's/^APP_PORT=.*/APP_PORT=:3001/' /opt/zenopanel/.env
        sed -i 's/^APP_TLS_PORT=.*/APP_TLS_PORT=:8443/' /opt/zenopanel/.env
        sed -i 's/^MGMT_PORT=.*/MGMT_PORT=:3002/' /opt/zenopanel/.env
    fi
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

# 9. Buat SHA-256 Checksum
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$OUTPUT_FILE" > "${OUTPUT_FILE}.sha256"
    log_success "Berkas checksum SHA-256 dibuat."
fi

# 10. Kompilasi Windows Launcher (.exe) berbasis Zig
LAUNCHER_FILE="dist/zenopanel-launcher.exe"
log_info "Mengompilasi Windows Launcher (.exe) berbasis Zig..."

# Update version in main.zig
sed -i 's|const VERSION = .*|const VERSION = "'"${VERSION}"'";|' launcher/main.zig

zig build-exe -target x86_64-windows-gnu -O ReleaseSmall -femit-bin="$LAUNCHER_FILE" launcher/main.zig
if [ $? -ne 0 ]; then
    log_error "Kompilasi Windows Launcher .exe gagal!"
    exit 1
fi
log_success "Windows Launcher .exe berhasil dikompilasi."

# 11. Mengemas ke berkas ZIP rilis (jika utility zip tersedia)
# ZIP rilis sekarang hanya berisi launcher.exe karena distro tarball akan diunduh dari GitHub
ZIP_NAME="zenopanel-windows-${VERSION}"
ZIP_FILE="dist/${ZIP_NAME}.zip"
HAS_ZIP=false
if command -v zip >/dev/null 2>&1; then
    log_info "Mengompresi launcher menjadi berkas ZIP siap pakai..."
    rm -f "$ZIP_FILE"
    zip -j "$ZIP_FILE" "$LAUNCHER_FILE" > /dev/null
    if [ $? -eq 0 ]; then
        HAS_ZIP=true
        log_success "Arsip ZIP rilis siap-pakai berhasil dibuat."
    fi
fi

# 12. Pembersihan Folder Temporer
log_info "Membersihkan direktori kerja..."
rm -rf "$BUILD_DIR" "$TEMP_EXTRACT"

# Selesai
log_success "Selamat! Pengemasan distro WSL2 berhasil diselesaikan!"
echo -e "\n${BOLD}Detail Hasil Akhir:${NC}"
if [ "$HAS_ZIP" = true ]; then
    echo -e "  - Berkas Rilis ZIP (Client): ${GREEN}${PWD}/${ZIP_FILE}${NC} (Hanya berisi launcher.exe)"
fi
echo -e "  - Berkas Tarball (GitHub)  : ${GREEN}${PWD}/${OUTPUT_FILE}${NC} (Unggah ke GitHub Releases)"
echo -e "  - Berkas Launcher (.exe)   : ${GREEN}${PWD}/${LAUNCHER_FILE}${NC}"
echo -e "  - Berkas Checksum          : ${GREEN}${PWD}/${OUTPUT_FILE}.sha256${NC}"
if [ "$HAS_ZIP" = true ]; then
    echo -e "  - Ukuran Paket ZIP         : ${GREEN}$(du -sh "${ZIP_FILE}" | cut -f1)${NC}"
fi
echo -e "=================================================="
echo -e "\n${BOLD}Langkah Rilis & Pengujian:${NC}"
echo -e "  1. Unggah berkas ${GREEN}$(basename "$OUTPUT_FILE")${NC} ke rilis GitHub Anda dengan tag ${YELLOW}${VERSION}${NC}."
echo -e "  2. Bagikan berkas ${GREEN}$(basename "$ZIP_FILE")${NC} ke pengguna Windows."
echo -e "  3. Saat pengguna mengekstrak dan menjalankan ${GREEN}zenopanel-launcher.exe${NC}, ia akan mengunduh distro secara otomatis dari GitHub."
echo -e "=================================================="



