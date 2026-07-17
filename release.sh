#!/bin/bash

# ==============================================================================
# 🚀 ZenoPanel GitHub Release Automation Tool
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
echo "      ZenoPanel GitHub Release Publisher          "
echo "=================================================="
echo -e "${NC}"

# 1. Periksa apakah GitHub CLI (gh) terpasang
if ! command -v gh >/dev/null 2>&1; then
    log_error "GitHub CLI (gh) tidak ditemukan. Silakan pasang 'gh' terlebih dahulu."
    exit 1
fi

# 2. Periksa status login gh
log_info "Memeriksa status autentikasi GitHub CLI..."
if ! gh auth status >/dev/null 2>&1; then
    log_error "Anda belum login ke GitHub CLI. Silakan jalankan 'gh auth login' di terminal terlebih dahulu."
    exit 1
fi
log_success "Autentikasi GitHub CLI valid."

# 3. Deteksi Versi ZenoPanel dari Cargo.toml atau Git
GIT_TAG=$(git describe --tags --abbrev=0 2>/dev/null)
if [ -n "$GIT_TAG" ]; then
    VERSION="$GIT_TAG"
else
    CARGO_VERSION=$(grep '^version =' Cargo.toml | head -n1 | cut -d '"' -f2 2>/dev/null)
    VERSION="v${CARGO_VERSION:-1.5.19}"
fi
log_info "Versi terdeteksi: ${BOLD}$VERSION${NC}"

# 4. Periksa apakah berkas rilis di folder dist/ sudah lengkap
TAR_FILE="dist/zenoos-${VERSION}.tar.gz"
SHA_FILE="dist/zenoos-${VERSION}.tar.gz.sha256"
ZIP_FILE="dist/zenopanel-windows-${VERSION}.zip"
LINUX_TAR_FILE="dist/zenopanel-${VERSION}.tar.gz"
LINUX_SHA_FILE="dist/zenopanel-${VERSION}.tar.gz.sha256"

# Periksa berkas Linux Standalone terlebih dahulu
if [ ! -f "$LINUX_TAR_FILE" ] || [ ! -f "$LINUX_SHA_FILE" ]; then
    log_warn "Berkas distribusi Linux Standalone ($LINUX_TAR_FILE) belum dibuat."
    read -p "Apakah Anda ingin mengompilasi ZenoPanel untuk Linux sekarang? (y/n, default y): " run_compile
    run_compile=${run_compile:-y}
    if [ "$run_compile" = "y" ] || [ "$run_compile" = "Y" ]; then
        log_info "Menjalankan ./compile.sh..."
        ./compile.sh --target musl --non-interactive
        if [ $? -ne 0 ]; then
            log_error "Kompilasi Linux Standalone gagal!"
            exit 1
        fi
    fi
fi

# Periksa berkas WSL & Launcher
MISSING_WSL=false
for FILE in "$TAR_FILE" "$SHA_FILE" "$ZIP_FILE"; do
    if [ ! -f "$FILE" ]; then
        log_error "Berkas WSL/Launcher tidak ditemukan: $FILE"
        MISSING_WSL=true
    fi
done

if [ "$MISSING_WSL" = true ]; then
    read -p "Apakah Anda ingin menjalankan skrip pengemasan './package_wsl.sh' sekarang? (y/n, default y): " run_pack
    run_pack=${run_pack:-y}
    if [ "$run_pack" = "y" ] || [ "$run_pack" = "Y" ]; then
        log_info "Menjalankan skrip pengemasan..."
        ./package_wsl.sh
        if [ $? -ne 0 ]; then
            log_error "Proses pengemasan WSL gagal!"
            exit 1
        fi
    else
        log_error "Batal mempublikasikan rilis karena berkas tidak lengkap."
        exit 1
    fi
fi

# Pastikan seluruh 5 berkas rilis ada
ALL_FILES_PRESENT=true
for FILE in "$TAR_FILE" "$SHA_FILE" "$ZIP_FILE" "$LINUX_TAR_FILE" "$LINUX_SHA_FILE"; do
    if [ ! -f "$FILE" ]; then
        log_error "Berkas rilis masih tidak ditemukan: $FILE"
        ALL_FILES_PRESENT=false
    fi
done

if [ "$ALL_FILES_PRESENT" = false ]; then
    log_error "Proses rilis dihentikan karena berkas aset tidak lengkap."
    exit 1
fi

# Fungsi untuk memanggil AI Gemini gratisan
generate_ai_release_notes() {
    if [ -z "$GEMINI_API_KEY" ]; then
        log_warn "GEMINI_API_KEY tidak ditemukan di environment."
        echo -e "Untuk menggunakan AI gratisan, Anda memerlukan API Key gratis dari Google AI Studio."
        echo -e "  1. Buka https://aistudio.google.com/"
        echo -e "  2. Buat API Key gratis."
        echo -e "  3. Masukkan kunci API Key Anda di bawah ini."
        echo -n "Masukkan API Key Anda sekarang (atau tekan Enter untuk batal): "
        read -r key_input
        if [ -n "$key_input" ]; then
            export GEMINI_API_KEY="$key_input"
        else
            log_warn "Batal menggunakan AI."
            return 1
        fi
    fi

    log_info "Mengambil riwayat commit Git..."
    PREV_TAG=$(git describe --tags --abbrev=0 "${VERSION}^" 2>/dev/null)
    if [ -z "$PREV_TAG" ]; then
        COMMITS=$(git log --oneline -n 30)
    else
        COMMITS=$(git log "${PREV_TAG}..HEAD" --oneline)
    fi

    if [ -z "$COMMITS" ]; then
        log_warn "Tidak ada commit baru terdeteksi sejak tag sebelumnya."
        return 1
    fi

    log_info "Meminta AI Gemini untuk menghasilkan catatan rilis..."

    JSON_PAYLOAD=$(python3 -c '
import json, sys
commits = sys.stdin.read()
prompt = f"Tulis catatan rilis (release notes) yang profesional, menarik, dan informatif berdasarkan riwayat commit berikut. Tulis dalam bahasa Indonesia dan format Markdown yang rapi. Kelompokkan berdasarkan kategori pembaruan, perbaikan bug, dan optimisasi performa:\n\n{commits}"
print(json.dumps({"contents": [{"parts": [{"text": prompt}]}]}))
' << EOF
$COMMITS
EOF
)

    AI_RESPONSE=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$JSON_PAYLOAD" \
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key=${GEMINI_API_KEY}")

    # Ekstrak teks respons
    release_notes=$(echo "$AI_RESPONSE" | python3 -c '
import json, sys
try:
    data = json.load(sys.stdin)
    if "candidates" in data and len(data["candidates"]) > 0:
        text = data["candidates"][0]["content"]["parts"][0]["text"]
        print(text)
    else:
        print("ERROR")
except:
    print("ERROR")
')

    if [ "$release_notes" = "ERROR" ] || [ -z "$release_notes" ]; then
        log_error "Gagal memproses respon dari AI Gemini."
        return 1
    fi

    # Gabungkan petunjuk instalasi di bagian atas
    release_notes="### ZenoPanel ${VERSION} Release

#### Cara Pemasangan di Windows
1. Unduh berkas \`zenopanel-windows-${VERSION}.zip\`.
2. Ekstrak berkas tersebut.
3. Jalankan \`zenopanel-launcher.exe\` untuk mengunduh distro ZenoOS dan menyalakan panel secara otomatis.

---
${release_notes}"

    log_success "Catatan rilis berhasil dibuat oleh AI Gemini!"
    return 0
}

# 5. Konfirmasi Tipe Rilis
echo -e "\nPilih tipe rilis GitHub:"
echo "1) Rilis Publik Langsung (Public Release)"
echo "2) Rilis Pra-Rilis (Pre-release)"
echo "3) Rilis Draf (Draft Release)"
read -p "Masukkan pilihan (1-3, default 1): " release_type_opt

RELEASE_FLAGS=""
RELEASE_STATUS="Publik"
case $release_type_opt in
    2)
        RELEASE_FLAGS="--prerelease"
        RELEASE_STATUS="Pre-release"
        ;;
    3)
        RELEASE_FLAGS="--draft"
        RELEASE_STATUS="Draf"
        ;;
    *)
        RELEASE_FLAGS=""
        RELEASE_STATUS="Publik"
        ;;
esac

# 6. Catatan Rilis (Release Notes)
echo -e "\nPilih metode pembuatan catatan rilis (Release Notes):"
echo "1) Hasilkan otomatis dari riwayat commit Git + Petunjuk Pemasangan (Rekomendasi)"
echo "2) Hanya hasilkan otomatis dari riwayat commit Git"
echo "3) Hanya gunakan templat standar ZenoPanel"
echo "4) Tulis kustom secara manual"
echo "5) Tulis pintar menggunakan AI Google Gemini (Gratis dengan API Key)"
echo "6) Gunakan isi dari berkas RELEASE_NOTES.md lokal (Default)"
read -p "Masukkan pilihan (1-6, default 6): " notes_opt
notes_opt=${notes_opt:-6}

GENERATE_NOTES_FLAG=""
release_notes=""

case $notes_opt in
    1)
        GENERATE_NOTES_FLAG="--generate-notes"
        release_notes="### ZenoPanel ${VERSION} Release

#### Cara Pemasangan di Windows
1. Unduh berkas \`zenopanel-windows-${VERSION}.zip\`.
2. Ekstrak berkas tersebut.
3. Jalankan \`zenopanel-launcher.exe\` untuk mengunduh distro ZenoOS dan menyalakan panel secara otomatis.

---
#### Changelog
"
        ;;
    2)
        GENERATE_NOTES_FLAG="--generate-notes"
        ;;
    3)
        release_notes="### ZenoPanel ${VERSION} Release

#### Apa yang Baru
- Optimisasi performa WSL 2 menggunakan VirtIO-FS dan Mirrored Network.
- Peralihan dari Go Launcher ke Native Zig Launcher (~200KB).
- Penambahan penanganan error WSL 2 dan notifikasi background saat startup.
- Menambahkan parameter \`--stop\` untuk mematikan layanan distro ZenoOS secara bersih.

#### Cara Pemasangan di Windows
1. Unduh berkas \`zenopanel-windows-${VERSION}.zip\`.
2. Ekstrak berkas tersebut.
3. Jalankan \`zenopanel-launcher.exe\` untuk mengunduh distro ZenoOS dan menyalakan panel secara otomatis."
        ;;
    4)
        echo -e "\nTulis deskripsi singkat untuk rilis ini:"
        read -p "> " release_notes
        ;;
    5)
        generate_ai_release_notes
        if [ $? -ne 0 ]; then
            log_warn "Gagal memproses dengan AI. Menggunakan templat standar sebagai cadangan..."
            release_notes="### ZenoPanel ${VERSION} Release

#### Apa yang Baru
- Optimisasi performa WSL 2 menggunakan VirtIO-FS dan Mirrored Network.
- Peralihan dari Go Launcher ke Native Zig Launcher (~200KB).
- Penambahan penanganan error WSL 2 dan notifikasi background saat startup.
- Menambahkan parameter \`--stop\` untuk mematikan layanan distro ZenoOS secara bersih.

#### Cara Pemasangan di Windows
1. Unduh berkas \`zenopanel-windows-${VERSION}.zip\`.
2. Ekstrak berkas tersebut.
3. Jalankan \`zenopanel-launcher.exe\` untuk mengunduh distro ZenoOS dan menyalakan panel secara otomatis."
        fi
        ;;
    6)
        if [ -f "RELEASE_NOTES.md" ]; then
            release_notes=$(cat RELEASE_NOTES.md)
            log_success "Berhasil memuat berkas RELEASE_NOTES.md lokal."
        else
            log_error "Berkas RELEASE_NOTES.md tidak ditemukan!"
            exit 1
        fi
        ;;
esac

# 7. Konfirmasi akhir sebelum mempublikasikan
echo -e "\n=================================================="
echo -e "Menyiapkan Rilis GitHub:"
echo -e "  - Repositori : $(git remote get-url origin)"
echo -e "  - Tag/Versi  : ${BOLD}${VERSION}${NC}"
echo -e "  - Tipe Rilis : ${BOLD}${RELEASE_STATUS}${NC}"
echo -e "  - Aset Rilis :"
echo -e "    * $TAR_FILE (ZenoOS WSL2 Distro)"
echo -e "    * $SHA_FILE"
echo -e "    * $ZIP_FILE (Windows Client Launcher)"
echo -e "    * $LINUX_TAR_FILE (Linux Standalone Core)"
echo -e "    * $LINUX_SHA_FILE"
echo -e "=================================================="
read -p "Apakah Anda yakin ingin mempublikasikan rilis ini? (y/n): " confirm_pub

if [ "$confirm_pub" != "y" ] && [ "$confirm_pub" != "Y" ]; then
    log_warn "Publikasi rilis dibatalkan oleh pengguna."
    exit 0
fi

# 8. Unggah dan buat Rilis di GitHub
log_info "Memulai pembuatan rilis di GitHub dan mengunggah aset..."

gh release create "$VERSION" \
    "$TAR_FILE" \
    "$SHA_FILE" \
    "$ZIP_FILE" \
    "$LINUX_TAR_FILE" \
    "$LINUX_SHA_FILE" \
    --title "ZenoPanel $VERSION" \
    --notes "$release_notes" \
    $RELEASE_FLAGS \
    $GENERATE_NOTES_FLAG

if [ $? -eq 0 ]; then
    echo ""
    log_success "Selamat! ZenoPanel ${VERSION} berhasil dirilis ke GitHub!"
    log_success "Tautan Rilis: $(git remote get-url origin | sed 's/\.git$//')/releases/tag/${VERSION}"
else
    log_error "Gagal membuat rilis di GitHub. Periksa koneksi internet Anda atau hak akses token repositori Anda."
    exit 1
fi
