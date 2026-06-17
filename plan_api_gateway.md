# Rencana Implementasi: Fitur API Gateway ZenoPanel

Dokumen ini menjelaskan rancangan arsitektur untuk meningkatkan modul Reverse Proxy ZenoPanel menjadi **API Gateway** yang komprehensif, memanfaatkan performa tinggi Cloudflare Pingora.

---

## User Review Required

> [!IMPORTANT]
> Fitur API Gateway akan memengaruhi skema database `proxy_rules` dan memerlukan penambahan callback baru pada implementasi `ProxyHttp` Pingora (`response_filter`).

---

## Open Questions

> [!WARNING]
> 1. **CORS & Auth pada Static Site**: Apakah fitur CORS dan autentikasi gateway juga harus berlaku untuk tipe rule `static` (static site hosting) atau hanya berlaku untuk `proxy` (reverse proxy)? *Usulan awal: Berlaku untuk keduanya.*
> 2. **Format UI Custom Headers**: Bagaimana format input untuk Custom Headers di modal UI? Apakah cukup menggunakan textarea dengan format baris `Key: Value` demi kesederhanaan, atau list dinamis? *Usulan awal: Textarea baris `Key: Value` untuk kesederhanaan implementasi.*

---

## Rancangan Perubahan

### 1. Database Schema (`proxy_rules`)
Kita akan menambahkan kolom baru pada tabel `proxy_rules` untuk menyimpan konfigurasi API Gateway:
* `cors_enabled` (INTEGER, 0 atau 1)
* `cors_origins` (TEXT, contoh: `*` atau `http://localhost:3000,https://my-app.com`)
* `custom_request_headers` (TEXT, format: `Key: Value` per baris)
* `custom_response_headers` (TEXT, format: `Key: Value` per baris)
* `gateway_auth_type` (TEXT, pilihan: `'none'`, `'api_key'`, `'jwt'`)
* `gateway_auth_secret` (TEXT, kunci JWT atau daftar valid API keys terpisah koma)

---

### 2. Backend Rust & Pingora Integration

#### [MODIFY] [proxyman.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/proxyman.rs)
* Perbarui struct `ProxyRule` dengan properti baru.
* Tambahkan query `ALTER TABLE` pada saat inisialisasi `ProxyManager::new()` untuk menambahkan kolom-kolom baru jika belum ada.
* Perbarui fungsi CRUD `add_rule` dan `update_rule` agar menerima dan menyimpan parameter baru tersebut.

#### [MODIFY] [proxy.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/proxy.rs)
* Perbarui mapper `proxy_rule_to_value` untuk menyertakan properti baru ke ZenoLang.
* Perbarui registrasi slot `proxy.add` dan `proxy.update` agar menerima parameter baru dari ZenoLang.

#### [MODIFY] [gateway.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/gateway.rs)
* **Autentikasi Gateway (`request_filter`)**:
  * Sebelum request diteruskan, jika rute memerlukan autentikasi (`api_key` / `jwt`), periksa kecocokan token di header `Authorization` atau query string.
  * Jika gagal/tidak sah, kembalikan respons `401 Unauthorized` secara langsung tanpa meneruskan ke upstream peer.
* **Header Request Injection (`upstream_request_filter`)**:
  * Urai `custom_request_headers` dan masukkan ke dalam header `upstream_request`.
* **CORS & Response Injection (`response_filter`)**:
  * Terapkan method `response_filter` pada `ZenoGateway`.
  * Jika `cors_enabled` aktif, tambahkan header:
    * `Access-Control-Allow-Origin: <origin-sesuai-list-atau-*>`
    * `Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS`
    * `Access-Control-Allow-Headers: Content-Type, Authorization, X-Requested-With`
  * Masukkan header dari `custom_response_headers` ke dalam response client.

#### [MODIFY] [main.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/main.rs)
* Terapkan logika autentikasi, penyuntikan header, dan CORS yang serupa di dalam handler static-serving di `main.rs` untuk menjamin konsistensi saat `rule_type == "static"`.

---

### 3. ZenoLang Routes

#### [MODIFY] [proxy.zl](file:///home/max/Documents/PROJ/github/zenopanel/zsrc/routes/proxy.zl)
* Perbarui parameter input pada endpoint `/api/proxy/add` dan `/api/proxy/update` agar meneruskan data baru ke slot `proxy.add` dan `proxy.update`.

---

### 4. Frontend UI & UX

#### [MODIFY] [modals.blade.zl](file:///home/max/Documents/PROJ/github/zenopanel/views/partials/modals.blade.zl)
* Tambahkan panel collapsible baru berlabel **"API Gateway Settings"** di dalam `add-proxy-modal`.
* Di dalamnya, tambahkan input-input berikut:
  * Checkbox: *Enable CORS Headers*
  * Input Text: *Allowed Origins* (muncul jika CORS aktif)
  * Select Option: *Gateway Authentication Type* (None, API Key, JWT)
  * Input Text/Password: *Auth Token Secret / API Keys* (muncul jika Auth Type bukan None)
  * Textarea: *Custom Request Headers* (format: `Header: Value`)
  * Textarea: *Custom Response Headers* (format: `Header: Value`)

#### [MODIFY] [proxy.js](file:///home/max/Documents/PROJ/github/zenopanel/public/js/proxy.js)
* Perbarui `submitAddProxy` dan event handler modal edit agar membaca data dari form input baru ini serta mengirimkannya dalam request JSON ke API backend.

---

## Rencana Verifikasi

### Skenario Pengujian Manual & Otomatis:
1. **Pengujian CORS**:
   * Kirim request preflight `OPTIONS` ke server dan verifikasi bahwa header CORS dikembalikan dengan benar.
2. **Pengujian Autentikasi**:
   * Kirim request tanpa API key pada rute yang dilindungi ➡️ harus mengembalikan `401 Unauthorized`.
   * Kirim request dengan API key yang benar ➡️ harus berhasil diteruskan ke upstream.
3. **Pengujian Custom Headers**:
   * Periksa apakah header request baru masuk ke aplikasi backend (upstream) dan header response kustom berhasil diterima oleh client di browser.
