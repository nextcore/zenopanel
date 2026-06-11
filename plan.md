# Plan - Modularize Rust Slot Management System

To clean up the monolithic `src/slots.rs` (2,200+ lines) and integrate other scattered slot modules (like `src/auth_slots.rs`), we will refactor the slot system into a modular directory structure under `src/slots/`.

## Proposed Structure

We will decompose the slots into logical categories:
```
src/
  slots/
    mod.rs         - Entrypoint; exports `register_custom_slots(engine: &mut Engine)`
    auth.rs        - Auth & cookie slots (`auth.guard`, `auth.is_admin`, `http.set_cookie`, etc.)
    db.rs          - Database slots (`db.select`, `db.execute`)
    http.rs        - HTTP-related response/request utilities (`http.ok`, `http.query`, etc.)
    io.rs          - File and Directory I/O slots (`io.file.read`, `io.file.archive`, etc.)
    proc.rs        - Managed processes slots (`proc.start`, `proc.logs`, etc.)
    proxy.rs       - Reverse proxy slots (`proxy.list`, `proxy.add`, etc.)
    system.rs      - System information and stats slots (`system.info`, `system.stats`, etc.)
    util.rs        - Core/utility functions (`coalesce`, `cast.to_int`, `fn`, etc.)
```

---

## Proposed Changes

### Rust Modules and Slots

#### [NEW] [mod.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/mod.rs)
- Defines the main registration entrance `pub fn register_custom_slots(engine: &mut Engine)` which calls:
  - `auth::register(engine)`
  - `db::register(engine)`
  - `http::register(engine)`
  - `io::register(engine)`
  - `proc::register(engine)`
  - `proxy::register(engine)`
  - `system::register(engine)`
  - `util::register(engine)`

#### [NEW] [auth.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/auth.rs)
- Relocated from `src/auth_slots.rs`.

#### [NEW] [db.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/db.rs)
- Relocated from `src/slots.rs` (`db.*`).

#### [NEW] [http.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/http.rs)
- Relocated from `src/slots.rs` (`http.*`).

#### [NEW] [io.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/io.rs)
- Relocated from `src/slots.rs` (`io.*`).

#### [NEW] [proc.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/proc.rs)
- Relocated from `src/slots.rs` (`proc.*`).

#### [NEW] [proxy.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/proxy.rs)
- Relocated from `src/slots.rs` (`proxy.*`).

#### [NEW] [system.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/system.rs)
- Relocated from `src/slots.rs` (`system.*`).

#### [NEW] [util.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots/util.rs)
- Relocated from `src/slots.rs` (`coalesce`, `cast.to_int`, `fn`, etc.).

#### [DELETE] [slots.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/slots.rs)
- Monolithic slot register file is removed.

#### [DELETE] [auth_slots.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/auth_slots.rs)
- Single-module auth slots file is removed.

---

### Integration Updates

#### [MODIFY] [main.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/main.rs)
- Update modules declaration:
  - Remove `mod slots;` and `mod auth_slots;`
  - Add `mod slots;` (which automatically loads `src/slots/mod.rs`).
- Update registration calls:
  - Remove `auth_slots::register_auth_slots(&mut engine);` (since it will be done inside `slots::register_custom_slots`).

---

## Verification Plan

### Automated Tests
- Run `cargo check` and `cargo build` to ensure modules compile properly.
- Run Python test suites:
  - `python3 scratch/test_auth.py`
  - `python3 scratch/test_settings.py`
  - `python3 scratch/test_multi_user.py`
  - `python3 scratch/verify.py`


Progress:

- `[x]` Create `/src/slots` directory
- `[x]` Implement utility/helpers file `src/slots/util.rs`
- `[x]` Implement auth file `src/slots/auth.rs`
- `[x]` Implement db file `src/slots/db.rs`
- `[x]` Implement http file `src/slots/http.rs`
- `[x]` Implement io file `src/slots/io.rs`
- `[x]` Implement proc file `src/slots/proc.rs`
- `[x]` Implement proxy file `src/slots/proxy.rs`
- `[x]` Implement system file `src/slots/system.rs`
- `[x]` Implement entrypoint `src/slots/mod.rs`
- `[ ]` Remove `src/slots.rs` and `src/auth_slots.rs`
- `[ ]` Update `src/main.rs` module declarations
- `[ ]` Compile project and run verification tests

