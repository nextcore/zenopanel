#!/bin/bash

# ==============================================================================
# ZenoPanel Auto-Installer Script
# Target URL: https://github.com/nextcore/zenopanel/dist/zenopanel-v0.0.9.tar.gz
# Description: Automates the setup, configuration, and service deployment of ZenoPanel.
# ==============================================================================

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Banner
print_banner() {
    clear
    echo -e "${CYAN}${BOLD}"
    echo "  _____                  _____                  _ "
    echo " |__  /___ _ __   ___   |  ___|_ _ _ __   ___  | |"
    echo "   / // _ \ '_ \ / _ \  | |_ / _\` | '_ \ / _ \ | |"
    echo "  / /|  __/ | | | (_) | |  _| (_| | | | |  __/ | |"
    echo " /____\___|_| |_|\___/  |_|  \__,_|_| |_|\___| |_|"
    echo "                                                  "
    echo -e "       ⚡ ZenoPanel Auto-Installer v0.0.9 ⚡"
    echo -e "${NC}"
}

# Logger helpers
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Root check
if [ "$EUID" -ne 0 ]; then
    log_error "Harap jalankan script ini sebagai root atau menggunakan sudo."
    exit 1
fi

print_banner

# 1. Dependency Check
log_info "Memeriksa dependensi sistem..."
DEPS=("curl" "tar" "openssl")
for dep in "${DEPS[@]}"; do
    if ! command -v "$dep" &> /dev/null; then
        log_info "Menginstal $dep..."
        if command -v apt-get &> /dev/null; then
            apt-get update -y && apt-get install -y "$dep"
        elif command -v yum &> /dev/null; then
            yum install -y "$dep"
        elif command -v dnf &> /dev/null; then
            dnf install -y "$dep"
        else
            log_error "Package manager tidak didukung. Harap instal '$dep' secara manual."
            exit 1
        fi
    fi
done
log_success "Semua dependensi terpenuhi."

# 2. Setup Variables
INSTALL_DIR="/opt/zenopanel"
TMP_DIR="/tmp/zeno_install"
TARBALL_URL="https://github.com/nextcore/zenopanel/dist/zenopanel-v0.0.9.tar.gz"

# 3. Detect existing installation
IS_UPDATE=false
if [ -d "$INSTALL_DIR" ] && [ -f "$INSTALL_DIR/zeno" ]; then
    IS_UPDATE=true
    log_warning "ZenoPanel terdeteksi sudah terinstal di $INSTALL_DIR."
    read -p "Apakah Anda ingin memperbarui (update) instalasi yang ada? (y/n): " confirm_update
    if [[ ! "$confirm_update" =~ ^[Yy]$ ]]; then
        log_info "Instalasi dibatalkan."
        exit 0
    fi
fi

# Ask configuration if not update
if [ "$IS_UPDATE" = false ]; then
    echo -e "\n${BOLD}--- Konfigurasi ZenoPanel ---${NC}"
    
    # Prompt Port
    read -p "Masukkan port untuk ZenoPanel Web (default: 3000): " APP_PORT
    APP_PORT=${APP_PORT:-3000}
    
    # Prompt TLS Port
    read -p "Masukkan port untuk HTTPS/TLS (default: 8443): " APP_TLS_PORT
    APP_TLS_PORT=${APP_TLS_PORT:-8443}

    # Prompt Admin Username
    read -p "Masukkan username Administrator (default: admin): " ADMIN_USERNAME
    ADMIN_USERNAME=${ADMIN_USERNAME:-admin}

    # Prompt Admin Password
    read -p "Masukkan password Administrator (kosongkan untuk generate otomatis): " ADMIN_PASSWORD
    if [ -z "$ADMIN_PASSWORD" ]; then
        ADMIN_PASSWORD=$(tr -dc 'A-Za-z0-9' </dev/urandom | head -c 12)
        GENERATE_PASSWORD=true
    else
        GENERATE_PASSWORD=false
    fi
fi

# 4. Download and Extract
log_info "Mengunduh paket ZenoPanel v0.0.9..."
rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"
if ! curl -sL "$TARBALL_URL" -o "$TMP_DIR/zenopanel.tar.gz"; then
    log_error "Gagal mengunduh ZenoPanel dari $TARBALL_URL"
    exit 1
fi

log_info "Mengekstrak arsip..."
tar -xzf "$TMP_DIR/zenopanel.tar.gz" -C "$TMP_DIR"

# Find extraction directory (handles nested folders inside tarball)
EXTRACT_PATH="$TMP_DIR"
if [ ! -f "$EXTRACT_PATH/zeno" ]; then
    SUBDIR=$(find "$EXTRACT_PATH" -mindepth 1 -maxdepth 1 -type d -name "zenopanel*" | head -n 1)
    if [ -n "$SUBDIR" ] && [ -f "$SUBDIR/zeno" ]; then
        EXTRACT_PATH="$SUBDIR"
    fi
fi

# Backup existing database and .env if update
if [ "$IS_UPDATE" = true ]; then
    log_info "Membuat backup konfigurasi dan database lama..."
    BACKUP_TIME=$(date +%Y%m%d%H%M%S)
    mkdir -p "$INSTALL_DIR/backup_$BACKUP_TIME"
    [ -f "$INSTALL_DIR/.env" ] && cp "$INSTALL_DIR/.env" "$INSTALL_DIR/backup_$BACKUP_TIME/.env"
    [ -f "$INSTALL_DIR/zeno.db" ] && cp "$INSTALL_DIR/zeno.db" "$INSTALL_DIR/backup_$BACKUP_TIME/zeno.db"
    
    # Remove files from extraction path that shouldn't overwrite existing installations
    rm -f "$EXTRACT_PATH/zeno.db"
    rm -f "$EXTRACT_PATH/.env"
fi

log_info "Menyalin file instalasi ke $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
cp -rf "$EXTRACT_PATH"/* "$INSTALL_DIR/" 2>/dev/null || true
if [ -f "$EXTRACT_PATH/.env.example" ]; then
    cp -f "$EXTRACT_PATH/.env.example" "$INSTALL_DIR/.env.example" 2>/dev/null || true
fi

# Ensure basic subdirectories exist
mkdir -p "$INSTALL_DIR/certs"
mkdir -p "$INSTALL_DIR/logs"

# Ensure binary is executable
chmod +x "$INSTALL_DIR/zeno"

# 5. Environment Configuration
log_info "Mengonfigurasi environment (.env)..."
if [ "$IS_UPDATE" = false ]; then
    # Create new .env from .env.example
    if [ -f "$INSTALL_DIR/.env.example" ]; then
        cp "$INSTALL_DIR/.env.example" "$INSTALL_DIR/.env"
    else
        # Fallback raw write .env if example is missing
        cat <<EOF > "$INSTALL_DIR/.env"
APP_PORT=:3000
APP_TLS_PORT=:8443
APP_ENV=production
DB_DRIVER=sqlite
DB_NAME=/opt/zenopanel/zeno.db
JWT_SECRET=
CSRF_ENABLED=true
CSRF_EXCEPT=/api,/health
EOF
    fi

    # Replace/Update key variables in .env
    sed -i "s|^APP_PORT=.*|APP_PORT=:$APP_PORT|" "$INSTALL_DIR/.env"
    sed -i "s|^APP_TLS_PORT=.*|APP_TLS_PORT=:$APP_TLS_PORT|" "$INSTALL_DIR/.env"
    sed -i "s|^APP_ENV=.*|APP_ENV=production|" "$INSTALL_DIR/.env"
    sed -i "s|^DB_NAME=.*|DB_NAME=/opt/zenopanel/zeno.db|" "$INSTALL_DIR/.env"
    
    # Generate secure JWT Secret
    JWT_SECRET=$(openssl rand -hex 32 2>/dev/null || tr -dc 'a-f0-9' </dev/urandom | head -c 64)
    sed -i "s|^JWT_SECRET=.*|JWT_SECRET=$JWT_SECRET|" "$INSTALL_DIR/.env"

    # Add Admin credentials to seed
    if grep -q "ADMIN_USERNAME=" "$INSTALL_DIR/.env"; then
        sed -i "s|^ADMIN_USERNAME=.*|ADMIN_USERNAME=$ADMIN_USERNAME|" "$INSTALL_DIR/.env"
    else
        echo "ADMIN_USERNAME=$ADMIN_USERNAME" >> "$INSTALL_DIR/.env"
    fi

    if grep -q "ADMIN_PASSWORD=" "$INSTALL_DIR/.env"; then
        sed -i "s|^ADMIN_PASSWORD=.*|ADMIN_PASSWORD=$ADMIN_PASSWORD|" "$INSTALL_DIR/.env"
    else
        echo "ADMIN_PASSWORD=$ADMIN_PASSWORD" >> "$INSTALL_DIR/.env"
    fi
else
    # Update logic: keep old config but ensure APP_ENV=production and DB path is absolute
    log_info "Mempertahankan file .env yang ada, melakukan penyesuaian minor jika diperlukan..."
    sed -i "s|^APP_ENV=.*|APP_ENV=production|" "$INSTALL_DIR/.env"
    # Ensure database path is absolute
    if grep -q "^DB_NAME=\./zeno.db" "$INSTALL_DIR/.env"; then
        sed -i "s|^DB_NAME=.*|DB_NAME=/opt/zenopanel/zeno.db|" "$INSTALL_DIR/.env"
    fi
fi

# 6. Systemd Service Registration
log_info "Mendaftarkan systemd service..."
cat <<EOF > /etc/systemd/system/zenopanel.service
[Unit]
Description=ZenoPanel Server Management Control Panel
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/zenopanel
ExecStart=/opt/zenopanel/zeno
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd daemon, enable on boot, and start/restart service
systemctl daemon-reload
systemctl enable zenopanel
systemctl restart zenopanel

# Get public or local IP address for dashboard access info
IP_ADDR=$(curl -s --max-time 3 https://ifconfig.me || curl -s --max-time 3 https://api.ipify.org || hostname -I | awk '{print $1}')

# Clean up temporary files
rm -rf "$TMP_DIR"

echo -e "\n${GREEN}${BOLD}==================================================${NC}"
if [ "$IS_UPDATE" = true ]; then
    log_success "ZenoPanel berhasil diperbarui ke v0.0.9!"
else
    log_success "ZenoPanel v0.0.9 berhasil diinstal!"
    echo -e "\n${BOLD}Detail Akses ZenoPanel:${NC}"
    echo -e "  - URL Panel:        ${CYAN}http://$IP_ADDR:$APP_PORT${NC}"
    echo -e "  - SSL URL Panel:    ${CYAN}https://$IP_ADDR:$APP_TLS_PORT${NC}"
    echo -e "  - Username Admin:   ${GREEN}$ADMIN_USERNAME${NC}"
    echo -e "  - Password Admin:   ${GREEN}$ADMIN_PASSWORD${NC}"
    if [ "$GENERATE_PASSWORD" = true ]; then
        echo -e "  ${YELLOW}* Harap simpan password di atas dengan aman!${NC}"
    fi
fi
echo -e "\n${BOLD}Perintah Manajemen Service:${NC}"
echo -e "  - Cek Status:  ${CYAN}systemctl status zenopanel${NC}"
echo -e "  - Restart:     ${CYAN}systemctl restart zenopanel${NC}"
echo -e "  - Stop:        ${CYAN}systemctl stop zenopanel${NC}"
echo -e "  - Cek Log:     ${CYAN}journalctl -u zenopanel -f${NC}"
echo -e "${GREEN}${BOLD}==================================================${NC}\n"
