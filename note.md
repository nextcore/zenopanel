# ZenoLang — Catatan Konsep & Visi Masa Depan

> Dibuat: 2026-07-01
> Konteks: Diskusi arsitektur ZenoLang dalam ekosistem Zeno Box / ZenoPanel

---

## 1. ZenoLang Sebagai Pengganti JSON & YAML

ZenoLang memiliki potensi untuk menggantikan JSON dan YAML sebagai format data/konfigurasi universal dalam ekosistem Zeno, berkat sejumlah keunggulan struktural:

| Fitur | JSON | YAML | ZenoLang |
|---|---|---|---|
| Komentar | ❌ | ✅ | ✅ |
| Tipe data eksplisit | Terbatas | Ambigus (`yes` = bool?) | ✅ |
| Indentation sensitif | ❌ | ✅ (sering jadi jebakan) | ❌ (block-based) |
| Logika kondisional | ❌ | ❌ | ✅ (`if:`, `else:`) |
| Variabel & referensi | ❌ | Anchors terbatas | ✅ native `$var` |
| Ekspresi | ❌ | ❌ | ✅ |
| Human-readable | Sedang | Baik | Baik |

### Contoh: Compose Definition dalam ZenoLang

```zenolang
# compose.zl — lebih bersih dari YAML, lebih ekspresif dari JSON
compose: 'my-laravel-app' {
    service: 'php-app' {
        image: 'webdevops/php-nginx:8.3-alpine'
        port: '8000:80'
        volume: '.:/app'
        env: {
            APP_ENV: 'production'
            APP_DEBUG: 'false'
            DB_PASSWORD: $env.DB_PASSWORD
        }
        memory: '512m'
        cpus: 0.5
    }

    service: 'mysql' {
        image: 'mysql:8.0'
        port: '3306:3306'
        env: {
            MYSQL_ROOT_PASSWORD: $env.DB_PASSWORD
            MYSQL_DATABASE: $env.DB_NAME
        }
        volume: 'mysql_data:/var/lib/mysql'
        restart: 'always'
    }

    volume: 'mysql_data'
}
```

### Model Transpilasi

ZenoLang bisa menjadi **single source of truth**, lalu di-*emit* ke format yang dibutuhkan downstream:

```
compose.zl
  ├──(ZenoLang Transpiler)──→  docker-compose.yml  →  Zeno Box (runc)
  ├──(ZenoLang Transpiler)──→  manifest.json        →  API response
  └──(ZenoLang Transpiler)──→  .env file            →  Runtime config
```

### ZenoLang Data Mode vs Imperative Mode

Untuk mewujudkan ini, ZenoLang perlu dua mode:

- **Imperative Mode** *(sudah ada)*: route handler, slot execution, logika server
- **Declarative/Data Mode** *(target masa depan)*: konfigurasi, manifest, compose definition — menghasilkan struktur data, bukan menjalankan aksi

---

## 2. ZenoLang di Dart — Cross-Platform Interpreter

### Visi

Dengan mengimplementasikan interpreter ZenoLang dalam **Dart**, maka via Flutter, ZenoLang dapat berjalan di semua platform secara native:

```
ZenoLang (.zl) + ZenoDart Interpreter + Flutter = Everywhere
```

| Platform | Target Runtime |
|---|---|
| 🌐 Web | Browser via WASM / JS |
| 📱 Android | Native AOT |
| 📱 iOS | Native AOT |
| 🖥️ macOS | Native Desktop |
| 🖥️ Windows | Native Desktop |
| 🖥️ Linux | Native Desktop |
| ⚙️ Server | Dart Server (AOT) |

### Arsitektur Sistem

```
                    ┌─────────────────────────────┐
                    │      ZenoLang (.zl files)    │
                    └─────────────┬───────────────┘
                                  │
               ┌──────────────────┼──────────────────┐
               ▼                  ▼                   ▼
    ┌──────────────────┐  ┌──────────────┐  ┌────────────────┐
    │  ZenoCore (Rust) │  │ ZenoDart     │  │  ZenoDart      │
    │  Server Engine   │  │  Interpreter │  │  Interpreter   │
    │  ZenoPanel       │  │  Flutter App │  │  Web App       │
    └──────────────────┘  └──────────────┘  └────────────────┘
           Server              Mobile/Desktop       Browser
```

### Kenapa Dart Adalah Pilihan Tepat

- **Parser/Lexer**: sangat mudah ditulis di Dart (strong typing, string handling bagus)
- **Dart AOT**: performa native di mobile, tidak ada VM overhead
- **Dart → WASM**: berjalan di browser tanpa JavaScript
- **`dart:ffi`**: bisa memanggil ZenoCore Rust library langsung untuk operasi berat
- **Flutter ecosystem**: widget, routing, state management sudah matang

### ZenoLang sebagai UI Engine (Flutter)

`.zl` file mendeskripsikan UI secara deklaratif, Dart me-*render*-nya sebagai Flutter Widget tree:

```zenolang
# screens/dashboard.zl
screen: 'Dashboard' {
    app_bar: { title: 'Zeno Box' }

    column: {
        widget: 'ServerStats' {
            refresh_interval: 5s
            source: api('/api/system/stats')
        }

        widget: 'ContainerList' {
            source: api('/api/containers/list')
            on_tap: navigate('ContainerDetail', id: $item.id)
        }

        fab: {
            icon: 'add'
            label: 'New Container'
            on_press: navigate('CreateContainer')
        }
    }
}
```

### Killer Feature: Server-Driven UI

ZenoPanel Flutter App tidak perlu di-*update* di setiap perangkat. Cukup update file `.zl` di server, dan semua client langsung mendapat tampilan baru **tanpa app store submission**:

```
ZenoBox Server
    └── /screens/*.zl  (diupdate kapan saja)
              ↓ HTTP fetch saat app launch
    Flutter App (ZenoDart interpreter)
              ↓ parse AST → render Flutter widgets
    Native UI — iOS, Android, Web, Desktop
```

Ini melampaui konsep React Server Components karena logika UI dan bisnis semuanya ada di `.zl` yang bisa diupdate *live* dari server.

---

## 3. Roadmap Implementasi ZenoDart (Usulan)

```
Phase 1: ZenoDart Parser
└── Lexer → Token stream
└── Parser → AST (Abstract Syntax Tree)
└── Mendukung: blok, string, angka, variabel, kondisi

Phase 2: ZenoDart Evaluator
└── Evaluasi ekspresi, variabel $var
└── Eksekusi blok if/else
└── Scope management

Phase 3: ZenoDart Built-in Slots (Dart-native)
└── http.get / http.post → Dart HTTP client
└── navigate → Flutter Navigator
└── api() → fetch dari ZenoCore server

Phase 4: Flutter Widget Mapper
└── Konversi AST node → Flutter Widget tree
└── screen: → Scaffold
└── column: → Column
└── widget: → Custom Widget dari registry

Phase 5: ZenoPanel Flutter App
└── Dashboard, Containers, Compose editor
└── Semua UI dari .zl yang di-fetch dari ZenoCore server
└── Offline fallback dengan bundled .zl cache
```

---

## 4. Ekosistem Zeno — Gambaran Jangka Panjang

```
┌──────────────────────────────────────────────────────────┐
│                    ZENO ECOSYSTEM                         │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  ZenoLang (.zl)  ←  Single language for everything      │
│       │                                                  │
│       ├── ZenoCore (Rust)     Server engine, slots        │
│       ├── ZenoBox             Container runtime (runc)    │
│       ├── ZenoBlade           Web framework               │
│       ├── ZenoDart (Flutter)  Cross-platform client       │
│       └── ZenoPanel           Admin dashboard             │
│                                                          │
│  Satu bahasa → Server, UI, Config, Orchestration         │
└──────────────────────────────────────────────────────────┘
```

ZenoLang berpotensi menjadi **lingua franca** untuk seluruh ekosistem Zeno: satu bahasa yang ditulis oleh developer, lalu dijalankan di manapun — server Rust, browser WASM, app Flutter mobile, atau desktop native.
