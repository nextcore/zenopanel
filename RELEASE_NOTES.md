# Release Notes — ZenoPanel v1.6.1

Rilis **v1.6.1** memperkuat kestabilan dan keamanan platform ZenoPanel dengan perbaikan penuh false positive pada **Zeno WAF**, serta menambahkan fitur lanjutan pada **Zeno Machine** mencakup **Cloud-Init Auto-Provisioning**, **Interactive Web Serial Console**, **1-Click Reverse Proxy Expose**, dan **Snapshot & State Checkpoint**.

---

## 🚀 Fitur Baru & Peningkatan (New Features & Enhancements)

### 🖥️ Zeno Machine — Cloud-Init Auto-Provisioning & Kredensial
*   **SSH Public Key Auto-Inject**: Pengguna kini dapat mengisikan SSH Public Key saat membuat Zeno Machine. ZenoPanel akan secara otomatis menyusun file `cloud-init/user-data` dan `meta-data` agar kunci SSH langsung diinjeksi saat MicroVM melakukan boot pertama kali.
*   **Custom Root/Initial User Password**: Menambahkan opsi penentuan password awal pengguna/root untuk Guest OS MicroVM.

### 💻 Interactive Web Serial Console (xterm.js)
*   **Akses TTY Konsol Langsung dari Browser**: Setiap Zeno Machine yang aktif kini dilengkapi tombol **Console** (`<i class="fa-solid fa-terminal"></i>`) untuk membuka **Web Serial Console** interaktif tanpa memerlukan SSH client eksternal.
*   **Terminal Emulation**: Menampilkan konsol TTY virtio-console secara real-time untuk memudahkan pengujian, perintah CLI, dan debugging sistem operasi Guest.

### 🌐 1-Click Expose via Reverse Proxy
*   **Reverse Proxy Binding Instant**: Cukup klik tombol **Expose Proxy** (`<i class="fa-solid fa-network-wired"></i>`) pada baris Zeno Machine untuk langsung membuat aturan Zeno Reverse Proxy yang mengarahkan domain/subdomain ke IP & port MicroVM.

### 📸 Machine Snapshot & State Checkpoint
*   **Checkpoint MicroVM**: Menambahkan tombol **Snapshot** (`<i class="fa-solid fa-camera"></i>`) untuk mengambil cadangan keadaan memori dan spesifikasi MicroVM yang tersimpan secara aman di `/var/lib/zeno-container/machines/snapshots/`.

---

## 🛡️ Perbaikan Keamanan & Bug Fixes

### 🛡️ Eliminasi False Positive Zeno WAF (WAF Overhaul)
*   **Fix Request Jegalan di Path `/`**: Memperbaiki `SQLI_REGEX` yang sebelumnya terlalu permisif pada kata `or`/`and` yang diikuti tanda `=` di header Cookie (`Cookie: session=...; order=desc`) atau Query Parameter.
*   **Fix Scanner Bot User-Agent Overlap**: Menambahkan *word boundaries* (`\b...\b`) ketat pada `SCANNER_UA_REGEX` untuk mencegah nama browser/app biasa yang memiliki substring serupa terdeteksi secara salah sebagai bot penyerang.
*   **Konsistensi Fast-Path Aho-Corasick**: Diselaraskan `WAF_KEYWORDS` dengan aturan regex baru. Seluruh 4 suite unit test WAF dipastikan lulus 100% (*4 passed, 0 failed*).

### 🎨 Perbaikan Layout UI & Integrasi JS Zeno Machine
*   **Layout Alignment & CSS Modals**: Memperbaiki perataan tampilan tab Zeno Machine agar simetris dengan dashboard utama, menyelaraskan `.btn-primary`, `.btn-secondary`, dan `.modal-backdrop`.
*   **HTMX Navigation Integration**: Menyarangkan pemanggilan `loadZenoMachines()` ke dalam siklus navigasi `runTabInit('machines')` pada `navigation.js` sehingga tab Zeno Machine selalu terisi otomatis saat diklik.

---

## 📦 Paket Distribusi & Rilis (Release Assets)

*   **`zenopanel-v1.6.1.tar.gz`**: Paket distribusi utama ZenoPanel v1.6.1 untuk sistem Linux, lengkap dengan binary `bin/cloud-hypervisor` v42.0 (static/musl).
*   **`zenopanel-windows-v1.6.1.zip`**: Paket launcher Windows (`zenopanel-launcher.exe` + `zenopanel.ps1`) dan distro ZenoOS berbasis Alpine minrootfs 3.24 untuk instalasi 1-klik di Windows WSL2.
