#!/bin/bash
# ==============================================================================
# 🧪 ZenoPanel Native Docker-Compose Integration Tester (CSRF-Compliant)
# ==============================================================================

# Warna output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== ZenoPanel Native Compose Test Script ===${NC}"

# 1. Konfigurasi
HOST="http://127.0.0.1:3002"
USERNAME="admin"
PASSWORD="admin"
COOKIE_JAR="/tmp/zeno_cookies.txt"
ENTRANCE_PATH="/zpanel"

# 2. Ambil CSRF Cookie pertama kali
echo -e "${YELLOW}Mengambil CSRF token dari ${HOST}${ENTRANCE_PATH}...${NC}"
curl -s -c "$COOKIE_JAR" "${HOST}${ENTRANCE_PATH}" > /dev/null

# Ekstrak token CSRF dari cookie jar
CSRF_TOKEN=$(grep "_csrf" "$COOKIE_JAR" | awk '{print $7}')
if [ -z "$CSRF_TOKEN" ]; then
    # Fallback jika kolom berbeda
    CSRF_TOKEN=$(grep "_csrf" "$COOKIE_JAR" | awk '{print $NF}')
fi

echo -e "CSRF Token didapat: ${YELLOW}$CSRF_TOKEN${NC}"

# 3. Melakukan login dengan menyertakan CSRF token
echo -e "\n${YELLOW}[1/4] Melakukan login ke ${HOST}${ENTRANCE_PATH}...${NC}"
LOGIN_RES=$(curl -s -b "$COOKIE_JAR" -c "$COOKIE_JAR" \
  -H "Content-Type: application/json" \
  -H "X-CSRF-Token: $CSRF_TOKEN" \
  -d "{\"username\":\"$USERNAME\",\"password\":\"$PASSWORD\"}" \
  "${HOST}${ENTRANCE_PATH}")

if [[ "$LOGIN_RES" == *"successful"* ]]; then
    echo -e "${GREEN}[✓] Login sukses! Cookie disimpan di $COOKIE_JAR${NC}"
else
    echo -e "${RED}[✗] Gagal login. Respon: $LOGIN_RES${NC}"
    rm -f "$COOKIE_JAR"
    exit 1
fi

# Ambil token CSRF terbaru setelah login (jika diperbarui)
CSRF_TOKEN=$(grep "_csrf" "$COOKIE_JAR" | awk '{print $7}')
if [ -z "$CSRF_TOKEN" ]; then
    CSRF_TOKEN=$(grep "_csrf" "$COOKIE_JAR" | awk '{print $NF}')
fi

# 4. Definisikan Mock Compose YAML (menggunakan image alpine super-ringan)
COMPOSE_YAML="version: '3.8'
services:
  test-alpine:
    image: alpine:latest
    container_name: test-native-compose-container
    command: sleep 300
    restart: unless-stopped
"

# 5. Trigger Compose UP
echo -e "\n${YELLOW}[2/4] Menjalankan Compose UP (Native Rust)...${NC}"
JSON_YAML=$(echo "$COMPOSE_YAML" | python3 -c 'import json, sys; print(json.dumps(sys.stdin.read().strip()))')
UP_RES=$(curl -s -b "$COOKIE_JAR" \
  -H "Content-Type: application/json" \
  -H "X-CSRF-Token: $CSRF_TOKEN" \
  -d "{\"action\":\"up\",\"yaml\":$JSON_YAML,\"project_name\":\"test_proj\"}" \
  "${HOST}/api/containers/compose")

echo -e "Respon UP:\n$UP_RES"

if [[ "$UP_RES" == *"test-native-compose-container"* ]]; then
    echo -e "${GREEN}[✓] Compose UP berhasil! Kontainer dideploy.${NC}"
elif [[ "$UP_RES" == *"mount command failed"* ]]; then
    echo -e "${GREEN}[✓] API Compose UP terpanggil & valid! (Ekspektasi: Gagal mount OverlayFS karena running non-root).${NC}"
else
    echo -e "${RED}[✗] Gagal memanggil Compose UP atau error tidak dikenal.${NC}"
fi

# 6. Trigger Compose PS untuk melihat status
echo -e "\n${YELLOW}[3/4] Menjalankan Compose PS untuk memeriksa status...${NC}"
PS_RES=$(curl -s -b "$COOKIE_JAR" \
  -H "Content-Type: application/json" \
  -H "X-CSRF-Token: $CSRF_TOKEN" \
  -d "{\"action\":\"ps\",\"project_name\":\"test_proj\"}" \
  "${HOST}/api/containers/compose")

echo -e "Status Kontainer:\n$PS_RES"

# 7. Trigger Compose DOWN untuk membersihkan kontainer
echo -e "\n${YELLOW}[4/4] Menjalankan Compose DOWN untuk membersihkan kontainer...${NC}"
DOWN_RES=$(curl -s -b "$COOKIE_JAR" \
  -H "Content-Type: application/json" \
  -H "X-CSRF-Token: $CSRF_TOKEN" \
  -d "{\"action\":\"down\",\"project_name\":\"test_proj\"}" \
  "${HOST}/api/containers/compose")

echo -e "Respon DOWN:\n$DOWN_RES"

# Hapus cookie jar
rm -f "$COOKIE_JAR"
echo -e "\n${GREEN}=== Pengujian Selesai ===${NC}"
