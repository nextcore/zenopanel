# Performance Improvements — Task List

## Phase 1: app.js
- [ ] Hapus `initPerformanceChart()`, `initTrafficChart()`, `startStatsPolling()` dari DOMContentLoaded
- [ ] Pindahkan semua compose functions ke dalam `functionsToBind` object
- [ ] Hapus blok manual window binding compose (lines 528-547)

## Phase 2: dashboard.js
- [ ] Tambah cache variables untuk semua 10 stat DOM elements
- [ ] Perluas `initRingElements()` untuk cache stat elements juga
- [ ] Hapus `initRingElements()` call dari `updateStatsUI()` (pakai cached refs langsung)
- [ ] Tambah `sessionStorage` cache di `loadStaticSystemInfo()`

## Phase 3: head.blade.zl
- [ ] Tambah `defer` ke `<script src="chart.js">`

## Phase 4: Sync ke dist
- [ ] Sync `app.js` → dist
- [ ] Sync `dashboard.js` → dist
- [ ] Sync `head.blade.zl` → dist

## Phase 5: Verifikasi
- [ ] Cek tidak ada error di console
- [ ] Verifikasi SSE hanya dibuka saat tab Dashboard aktif
