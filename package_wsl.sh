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

# 1. Deteksi Berkas Rilis ZenoPanel di /dist
log_info "Mencari paket distribusi ZenoPanel di folder dist/..."
TAR_FILE=$(find dist -maxdepth 1 -name "zenopanel-*.tar.gz" ! -name "*windows*" ! -name "*wsl*" | head -n 1)

if [ -z "$TAR_FILE" ]; then
    log_error "Tidak menemukan paket ZenoPanel (dist/zenopanel-*.tar.gz)."
    log_error "Harap jalankan './compile.sh' terlebih dahulu untuk membuat paket kompilasi."
    exit 1
fi

log_success "Ditemukan paket ZenoPanel: ${BOLD}$TAR_FILE${NC}"

# Ekstrak versi dari nama file
VERSION=$(basename "$TAR_FILE" | sed -E 's/zenopanel-(v?[0-9]+\.[0-9]+\.[0-9]+)\.tar\.gz/\1/')
log_info "Mendeteksi versi ZenoPanel: ${BOLD}$VERSION${NC}"

# 2. Cari & Unduh Alpine Minrootfs Terbaru
log_info "Mencari versi Alpine Linux stabil terbaru..."
INDEX_HTML=$(curl -sL https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/)
ALPINE_FILE=$(echo "$INDEX_HTML" | grep -oE 'alpine-minirootfs-[0-9]+\.[0-9]+\.[0-9]+-x86_64\.tar\.gz' | head -n1)

if [ -z "$ALPINE_FILE" ]; then
    log_warn "Gagal mendeteksi rilis otomatis di CDN. Menggunakan fallback versi Alpine 3.24.1..."
    ALPINE_FILE="alpine-minirootfs-3.24.1-x86_64.tar.gz"
fi

ALPINE_URL="https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/$ALPINE_FILE"
log_success "Menggunakan Alpine minrootfs: ${BOLD}$ALPINE_FILE${NC}"

# Unduh Alpine Minrootfs dengan Caching
CACHE_DIR="dist/cache"
mkdir -p "$CACHE_DIR"

if [ ! -f "$CACHE_DIR/$ALPINE_FILE" ]; then
    log_info "Mengunduh Alpine minrootfs dari CDN..."
    curl -L -# -o "$CACHE_DIR/$ALPINE_FILE" "$ALPINE_URL"
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
TEMP_EXTRACT="dist/zeno_temp"

log_info "Membersihkan sisa-sisa build lama..."
rm -rf "$BUILD_DIR" "$TEMP_EXTRACT"
mkdir -p "$BUILD_DIR" "$TEMP_EXTRACT"

# 4. Ekstrak Alpine Minrootfs
log_info "Mengekstrak Alpine minrootfs..."
tar -xzf "$CACHE_DIR/$ALPINE_FILE" -C "$BUILD_DIR"
if [ $? -ne 0 ]; then
    log_error "Gagal mengekstrak Alpine minrootfs."
    exit 1
fi
log_success "Ekstraksi Alpine berhasil."

# 5. Ekstrak ZenoPanel dan Tempatkan ke /opt/zenopanel
log_info "Mengekstrak paket ZenoPanel..."
tar -xzf "$TAR_FILE" -C "$TEMP_EXTRACT"
EXTRACT_SUBFOLDER=$(ls "$TEMP_EXTRACT" | head -n 1)

log_info "Memindahkan berkas ZenoPanel ke /opt/zenopanel..."
mkdir -p "$BUILD_DIR/opt/zenopanel"
cp -r "$TEMP_EXTRACT/$EXTRACT_SUBFOLDER"/* "$BUILD_DIR/opt/zenopanel/"

# Copy .env.example jika ada sebagai dasar .env
if [ -f "$TEMP_EXTRACT/$EXTRACT_SUBFOLDER/.env.example" ]; then
    cp "$TEMP_EXTRACT/$EXTRACT_SUBFOLDER/.env.example" "$BUILD_DIR/opt/zenopanel/.env"
fi

# Pastikan izin eksekusi untuk binary
chmod +x "$BUILD_DIR/opt/zenopanel/zeno"

# 6. Buat Direktori Data Tambahan
log_info "Mempersiapkan direktori container runtime (Zeno Box)..."
mkdir -p "$BUILD_DIR/var/lib/zeno-container"
mkdir -p "$BUILD_DIR/run/openrc" # Untuk bypass deteksi OpenRC awal jika diperlukan

# 7. Tambahkan Helper/Launcher script di /usr/local/bin
log_info "Membuat skrip pembantu (launcher) di /usr/local/bin/zenopanel..."
cat << 'EOF' > "$BUILD_DIR/usr/local/bin/zenopanel"
#!/bin/sh
# Launcher ZenoPanel untuk WSL2

cd /opt/zenopanel || exit 1

# Generate JWT_SECRET otomatis jika belum ada di .env
if [ -f .env ] && ! grep -q "^JWT_SECRET=" .env; then
    echo "[ZenoPanel] Menginisialisasi JWT_SECRET baru di .env..."
    ./zeno key:generate >/dev/null 2>&1
fi

exec ./zeno "$@"
EOF

chmod +x "$BUILD_DIR/usr/local/bin/zenopanel"
ln -sf /opt/zenopanel/zeno "$BUILD_DIR/usr/local/bin/zeno"

# 8. Kemas Ulang sebagai Distro WSL2 Windows
OUTPUT_NAME="zenopanel-windows-${VERSION}"
OUTPUT_FILE="dist/${OUTPUT_NAME}.tar.gz"

log_info "Mengompresi distro WSL2 kustom..."
rm -f "$OUTPUT_FILE" "${OUTPUT_FILE}.sha256"

# Buat tar.gz
cd "$BUILD_DIR" || exit 1
tar -czf "../${OUTPUT_NAME}.tar.gz" .
cd - > /dev/null || exit 1

# 9. Buat SHA-256 Checksum
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$OUTPUT_FILE" > "${OUTPUT_FILE}.sha256"
    log_success "Berkas checksum SHA-256 dibuat."
fi

# 10. Buat Windows Launcher Batch Script
LAUNCHER_FILE="dist/zenopanel-launcher.bat"
log_info "Membuat Windows Launcher (.bat)..."

# Buat berkas batch launcher dengan carriage return (\r\n) agar kompatibel dengan Windows CMD
cat << EOF | sed 's/$/\r/' > "$LAUNCHER_FILE"
@echo off
set DISTRO_NAME=zenopanel
set TAR_FILE=zenopanel-windows-${VERSION}.tar.gz
set INSTALL_DIR=%%LOCALAPPDATA%%\zenopanel-wsl

:: 1. Cek apakah distro 'zenopanel' sudah terdaftar di WSL
wsl -l | findstr /I /C:"%%DISTRO_NAME%%" >nul
if %%errorlevel%% neq 0 (
    echo ==================================================
    echo       ZenoPanel Windows Automated Installer
    echo ==================================================
    echo Distro '%%DISTRO_NAME%%' belum terdaftar di WSL 2.
    echo Memulai proses instalasi otomatis...
    echo.
    
    :: Cek apakah file tarball ada di folder yang sama
    if not exist "%%~dp0%%TAR_FILE%%" (
        echo [ERROR] Berkas %%TAR_FILE%% tidak ditemukan di folder ini!
        echo Pastikan berkas %%TAR_FILE%% berada di folder yang sama dengan launcher ini.
        echo.
        pause
        exit /b 1
    )
    
    :: Buat direktori tujuan di AppData Local
    mkdir "%%INSTALL_DIR%%" 2>nul
    
    echo Mengimpor distro ke WSL 2 (ini memerlukan waktu beberapa detik)...
    wsl --import %%DISTRO_NAME%% "%%INSTALL_DIR%%" "%%~dp0%%TAR_FILE%%" --version 2
    if %%errorlevel%% neq 0 (
        echo [ERROR] Gagal mengimpor distro ke WSL 2. Pastikan WSL 2 sudah aktif.
        echo.
        pause
        exit /b 1
    )
    echo Distro berhasil diimpor!
    echo ==================================================
    echo.
)

:: 2. Siapkan direktori dan file launcher permanent di AppData
mkdir "%%INSTALL_DIR%%" 2>nul
set VBS_FILE=%%INSTALL_DIR%%\zenopanel-run.vbs

:: Tulis file VBScript permanent
echo Set WshShell = CreateObject("WScript.Shell") > "%%VBS_FILE%%"
echo WshShell.Run "wsl -d %%DISTRO_NAME%% -u root --cd /opt/zenopanel /usr/local/bin/zenopanel", 0, False >> "%%VBS_FILE%%"
echo Wscript.Sleep 1000 >> "%%VBS_FILE%%"
echo WshShell.Run "cmd.exe /c start http://localhost:3001/zpanel", 0, False >> "%%VBS_FILE%%"

:: 3. Buat Shortcut Cantik di Desktop jika belum ada
set SHORTCUT_PATH=%%USERPROFILE%%\Desktop\ZenoPanel.lnk
if not exist "%%SHORTCUT_PATH%%" (
    echo Membuat shortcut ZenoPanel di Desktop Anda...
    powershell -Command "\$WshShell = New-Object -ComObject WScript.Shell; \$Shortcut = \$WshShell.CreateShortcut('%%SHORTCUT_PATH%%'); \$Shortcut.TargetPath = 'wscript.exe'; \$Shortcut.Arguments = '\"%%VBS_FILE%%\"'; \$Shortcut.IconLocation = 'imageres.dll,-1005'; \$Shortcut.Description = 'ZenoPanel Server Control Center'; \$Shortcut.Save()"
    echo Shortcut berhasil dibuat di Desktop!
    echo.
)

:: 4. Jalankan ZenoPanel
echo Menjalankan server ZenoPanel di background WSL 2...
wscript.exe "%%VBS_FILE%%"

echo Berhasil! Halaman dashboard akan terbuka di browser Anda.
timeout /t 3 >nul
EOF

log_success "Windows Launcher .bat berhasil dibuat."

# 11. Mengemas ke berkas ZIP rilis (jika utility zip tersedia)
ZIP_FILE="dist/${OUTPUT_NAME}.zip"
HAS_ZIP=false
if command -v zip >/dev/null 2>&1; then
    log_info "Mengompresi paket distribusi menjadi berkas ZIP siap pakai..."
    rm -f "$ZIP_FILE"
    zip -j "$ZIP_FILE" "$OUTPUT_FILE" "$LAUNCHER_FILE" > /dev/null
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
    echo -e "  - Berkas Rilis ZIP: ${GREEN}${PWD}/${ZIP_FILE}${NC}"
fi
echo -e "  - Berkas Tarball  : ${GREEN}${PWD}/${OUTPUT_FILE}${NC}"
echo -e "  - Berkas Launcher : ${GREEN}${PWD}/${LAUNCHER_FILE}${NC}"
echo -e "  - Berkas Checksum : ${GREEN}${PWD}/${OUTPUT_FILE}.sha256${NC}"
if [ "$HAS_ZIP" = true ]; then
    echo -e "  - Ukuran Paket ZIP: ${GREEN}$(du -sh "${ZIP_FILE}" | cut -f1)${NC}"
else
    echo -e "  - Ukuran Tarball  : ${GREEN}$(du -sh "${OUTPUT_FILE}" | cut -f1)${NC}"
fi
echo -e "=================================================="
echo -e "\n${BOLD}Cara Menjalankan Sekali Klik di Windows:${NC}"
if [ "$HAS_ZIP" = true ]; then
    echo -e "  1. Ekstrak berkas ${GREEN}$(basename "$ZIP_FILE")${NC} di Windows."
    echo -e "  2. Double-click file ${GREEN}zenopanel-launcher.bat${NC}."
else
    echo -e "  1. Salin berkas ${GREEN}$(basename "$OUTPUT_FILE")${NC} dan ${GREEN}zenopanel-launcher.bat${NC} ke satu folder yang sama di Windows."
    echo -e "  2. Double-click file ${GREEN}zenopanel-launcher.bat${NC}."
fi
echo -e "  * Script akan menginstal distro secara otomatis pada klik pertama, lalu langsung membuka dashboard."
echo -e "=================================================="

