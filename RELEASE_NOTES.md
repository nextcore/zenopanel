# 🚀 ZenoPanel Release Notes — v1.7.0

Kami dengan bangga mengumumkan rilis **ZenoPanel v1.7.0**! 

Versi ini membawa peningkatan besar pada fondasi *scripting & template engine* dengan mengintegrasikan **`zenocore v0.2.0`** resmi dari [crates.io](https://crates.io/crates/zenocore), menghadirkan **50+ slot standar baru**, serta memperkenalkan dukungan **Native C-ABI Dynamic Plugin System**.

---

## 🌟 Sorotan Utama Rilis v1.7.0

### 1. 📦 Integrasi `zenocore v0.2.0` dari crates.io
ZenoPanel secara resmi bermigrasi ke paket **`zenocore v0.2.0`**, **`zeno-blade v0.2.0`**, **`zeno-std v0.2.0`**, dan **`zenoengine v0.2.0`** yang diterbitkan langsung pada registry crates.io:
- Menjamin stabilitas ekosistem dan kompatibilitas jangka panjang.
- Arsitektur interior mutability `Mutex` yang *thread-safe* untuk penanganan *concurrent HTTP request* di Tokio/Axum.

### 2. 🧰 Suite 50+ Slot Bawaan (Standard Library)
Seluruh skrip & template Blade di ZenoPanel kini mendapatkan akses penuh ke suite slot bawaan ZenoCore:
- **String Manipulation (`string.*`)**: `trim`, `upper`, `lower`, `split`, `replace`, `contains`, `starts_with`, `ends_with`, `len`, `concat`, `substr`, `format`, `repeat`.
- **Kalkulasi Matematika (`math.*`)**: `add`, `sub`, `mul`, `div`, `mod`, `pow`, `sqrt`, `abs`, `ceil`, `floor`, `round`, `min`, `max`, `clamp`, `random`.
- **Koleksi Array & Map (`array.*` / `map.*`)**: `reverse`, `unique`, `shift`, `unshift`, `slice`, `contains`, `sort`, `map.get`, `delete`, `merge`, `values`, `has`, `entries`.
- **Evaluasi Logika Dinamis**: Pencabangan `if` dengan operator logika kompleks (`&&`, `||`, ternary `? :`, dan null-coalescing `??`).
- **Tipe Data & Utilitas**: `cast.to_int/float/string/bool`, `coalesce`, `util.datetime`, `util.timestamp`, `util.uuid`, `util.env`.

### 3. 🔌 Native Dynamic Plugin Engine (`plugin.load`)
ZenoPanel kini dapat memuat plugin `.so` (Linux), `.dylib` (macOS), atau `.dll` (Windows) yang dikompilasi dari Rust secara *runtime* tanpa perlu mengompilasi ulang biner ZenoPanel utama:
```yaml
# Memuat extension plugin eksternal secara dinamis
plugin.load: './plugins/libcustom_waf.so'
```

---

## 🛠️ Ringkasan Perubahan Teknis (Changelog)

- **[Dependency]**: Upgraded `zenocore`, `zeno-blade`, `zeno-std`, `zenoengine` to `0.2.0`.
- **[Engine]**: Refactored `Engine` struct to use `Mutex` guards for slot registry and documentation.
- **[Feature]**: Added dynamic slot loading support via `libloading` FFI.
- **[Parity]**: Completed full 1:1 feature parity between ZenoCore stdlib and ZenoPanel templates.
- **[Build]**: Verified clean compilation across Rust 2024 edition target.

---

## 📥 Cara Update

```bash
cd zenopanel
git pull origin main
cargo build --release
```

Terima kasih kepada seluruh kontributor dan komunitas NextCore yang terus mendukung perkembangan ZenoPanel & ZenoLang! 🎉
