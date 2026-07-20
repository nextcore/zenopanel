# Release Notes — ZenoPanel v1.6.2

Rilis **v1.6.2** memberikan perbaikan penting pada pengolahan template UI Zeno Machine (Blade Parser Fix), mengatasi kendala `unclosed zeno` yang sempat menghambat respon navigasi tab side menu, menyelaraskan versi paket kompilasi distribusi utama Linux dan installer WSL2 Windows, serta **menambahkan fitur Manajemen ISO dan Snapshot Zeno Machine**.

---

## 🚀 Fitur Baru (New Features)

### 📀 Zeno Machine — Manajemen ISO (ISO Image Library)
*   **ISO Library & Manager Modal**: Menyediakan antarmuka terpusat untuk mendaftarkan, menyimpan, dan mengelola file ISO image (misal Ubuntu, Debian, Alpine) di `/var/lib/zeno-container/isos/`.
*   **Attach / Detach ISO Boot**: Mendukung pemilihan ISO image boot saat membuat Zeno Machine (MicroVM) baru maupun mengaitkan/melepas ISO pada VM yang sudah terdaftar.
*   **API Management ISO**: Menambahkan rute API `/api/machines/isos/list`, `/api/machines/isos/add`, `/api/machines/isos/delete`, `/api/machines/isos/attach`, dan `/api/machines/isos/detach`.

### 📸 Zeno Machine — Manajemen Snapshot (Snapshot Manager)
*   **Snapshot Manager UI**: Menyediakan modal manajemen snapshot khusus per Zeno Machine untuk melihat riwayat snapshot, membuat snapshot baru dengan nama & deskripsi kustom, diiringi status badge visual.
*   **Restore & Delete Snapshot**: Mendukung fitur restore status Zeno Machine dari file snapshot yang dipilih serta opsi penghapusan file snapshot.
*   **API Management Snapshot**: Menambahkan rute API `/api/machines/snapshots/list`, `/api/machines/snapshots/create`, `/api/machines/snapshots/restore`, dan `/api/machines/snapshots/delete`.

---

## 🛠️ Perbaikan Bug & Stabilitas (Bug Fixes & Stability)

### 🖥️ Zeno Machine — Fix Blade Template Parser & Navigasi Tab Side Menu
*   **Fix Error `unclosed zeno` di Terminal & UI**: Memperbaiki masalah di mana saat tombol **Zeno Machine** pada side menu diklik, tidak terjadi pergerakan halaman di UI dan terminal mengeluarkan log error `unclosed zeno`.
*   **Escaping `@` pada TTY Console Preview**: Karakter `@` pada string prompt konsol TTY (`root@zeno-vm:~#` dan `user@host`) di `views/partials/tab_machines.blade.zl` diubah menggunakan HTML Entity (`&#64;`). Hal ini mencegah parser **Zeno-Blade** salah menginterpretasikan string tersebut sebagai klausa/direktif Blade bertingkat (`@zeno`), sehingga rendering HTMX dan navigasi tab kembali berjalan lancar 100%.

---

## 📦 Paket Distribusi & Rilis (Release Assets)

*   **`zenopanel-v1.6.2.tar.gz`**: Paket distribusi rilis ZenoPanel v1.6.2 untuk sistem Linux (static musl binary), lengkap dengan binary `bin/cloud-hypervisor` v42.0.
*   **`zenopanel-windows-v1.6.2.zip`**: Paket installer/launcher Windows WSL2 (`zenopanel-launcher.exe` + `zenopanel.ps1`) dan distro ZenoOS berbasis Alpine minrootfs 3.24.

