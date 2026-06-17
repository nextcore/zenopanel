# 🛠️ Panduan Eksekusi & Konfigurasi Development ZenoPanel

Dokumen ini berisi panduan untuk menjalankan ZenoPanel di lingkungan lokal (development) serta penjelasan mengenai konfigurasi port dan lingkungan kerja.

---

## 🚀 Menjalankan ZenoPanel di Lingkungan Development

Untuk menjalankan ZenoPanel secara lokal dengan mode development, Anda cukup menggunakan perintah `cargo run` standar. Namun, karena dependensi native (seperti Pingora dan komponen SSL/kriptografi) membutuhkan CMake versi terbaru saat build, ikuti langkah berikut:

### Perintah Utama (Dev):
```bash
PATH=$PWD/cmake_local/bin:$PATH cargo run
```

### Mengapa membutuhkan `PATH=$PWD/cmake_local/bin:$PATH`?
1. **Self-contained Build Tools**: Beberapa crate Rust pendukung Pingora (misalnya `aws-lc-sys` atau `openssl-sys`) memerlukan utilitas **CMake** saat proses kompilasi kode native C/C++.
2. **Menghindari Konflik Versi**: Mesin host Anda mungkin belum memiliki CMake atau memiliki versi yang tidak kompatibel. Proyek ini menyertakan binary CMake yang kompatibel di dalam direktori `cmake_local/bin`.
3. **Penyuntikan PATH Sementara**: Sintaks di atas menambahkan direktori CMake lokal tersebut ke dalam env `PATH` hanya selama perintah `cargo run` berlangsung.

---

## ⚙️ Konfigurasi Port di File `.env`

ZenoPanel mengelola lalu lintas web dan antarmuka kontrol admin melalui tiga port utama yang didefinisikan di dalam file `.env`:

| Variabel Env | Contoh Nilai | Fungsi |
| :--- | :--- | :--- |
| `APP_PORT` | `:3001` (Dev) / `:80` (Prod) | Port proxy utama untuk lalu lintas web non-secure (HTTP). |
| `APP_TLS_PORT` | `:8443` (Dev) / `:443` (Prod) | Port proxy utama untuk lalu lintas web secure (HTTPS). |
| `MGMT_PORT` | `:3002` (Dev / Prod) | Port internal untuk server admin/Web UI (Axum). |

---

## 🌐 Perbedaan Lingkungan Development vs. Produksi

### 1. Lingkungan Development (Lokal)
Di komputer lokal Anda, disarankan menggunakan port non-privilege di atas 1024 agar Anda tidak memerlukan akses superuser (`sudo`) saat menjalankan program:
```env
APP_PORT=:3001
APP_TLS_PORT=:8443
MGMT_PORT=:3002
```
Akses Web UI secara lokal dapat dilakukan via: `http://localhost:3001/zpanel` (request masuk ke port proxy `3001` lalu diteruskan ke port admin `3002`).

### 2. Lingkungan Produksi (Production Server)
Di server produksi, website harus dapat diakses langsung tanpa menuliskan port pada URL. Oleh karena itu, Pingora gateway harus mengikat port standar web:
```env
APP_PORT=:80
APP_TLS_PORT=:443
MGMT_PORT=:3002
```
> [!IMPORTANT]
> Port di bawah 1024 membutuhkan hak akses administrator. Di produksi, Anda harus mem-build binary-nya (`cargo build --release`) lalu menjalankannya sebagai root (`sudo`) atau memberikan kapabilitas jaringan:
> ```bash
> sudo setcap CAP_NET_BIND_SERVICE=+ep target/release/zeno
> ```

---

## 🔒 Cara Mengakses Web UI ZenoPanel

Anda **tidak perlu mengakses port `3002` (`MGMT_PORT`) secara langsung** di browser, dan Anda **sangat dilarang membuka port `3002` pada firewall server**.

Gateway Pingora yang mendengarkan di port `80`/`443` (atau port dev `3001`/`8443`) telah dikonfigurasi untuk melakukan reverse-routing secara otomatis:
1. Pengguna membuka URL: `https://domain-anda.com/zpanel` (Path masuk khusus admin).
2. Pingora mendeteksi path `/zpanel` dan melompati rute ke luar untuk meneruskannya langsung secara internal ke `127.0.0.1:3002`.
3. UI kontrol panel dimuat dengan aman tanpa mengekspos port internalnya ke internet.
