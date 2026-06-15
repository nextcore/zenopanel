import { getCSRFToken, formatBytes, escapeHtml } from './utils.js';
import { showToast } from './toast.js';

export let currentFilePath = '.';
export let activeEditorPath = '';
export let fmCurrentData = [];       // cached file list for re-sort
export let fmSortKey   = 'name';     // active sort column
export let fmSortAsc   = true;       // ascending = true

// ---- Sort helpers ----
export function setSortBy(key) {
    if (fmSortKey === key) {
        fmSortAsc = !fmSortAsc;   // toggle direction
    } else {
        fmSortKey = key;
        // Default direction: asc for name/type, desc for size/time
        fmSortAsc = (key === 'name' || key === 'type');
    }
    updateSortHeaders();
    renderFileRows(fmCurrentData);
}

export function updateSortHeaders() {
    const cols = { name: 'th-name', size: 'th-size', type: 'th-type', mode: 'th-mode', mod_time: 'th-mod' };
    const icons = { name: 'fa-sort', size: 'fa-sort', type: 'fa-sort', mode: 'fa-sort', mod_time: 'fa-sort' };
    Object.entries(cols).forEach(([k, id]) => {
        const th = document.getElementById(id);
        if (!th) return;
        th.classList.remove('sort-asc', 'sort-desc');
        const icon = th.querySelector('.sort-icon');
        if (k === fmSortKey) {
            th.classList.add(fmSortAsc ? 'sort-asc' : 'sort-desc');
            if (icon) {
                icon.className = `fa-solid ${fmSortAsc ? 'fa-sort-up' : 'fa-sort-down'} sort-icon`;
            }
        } else {
            if (icon) icon.className = 'fa-solid fa-sort sort-icon';
        }
    });
}

export function sortData(data) {
    return [...data].sort((a, b) => {
        // Folders always first
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;

        let va, vb;
        if (fmSortKey === 'name')     { va = a.name.toLowerCase(); vb = b.name.toLowerCase(); }
        else if (fmSortKey === 'size') { va = a.size; vb = b.size; }
        else if (fmSortKey === 'type') { va = a.is_dir ? 'dir' : a.name.split('.').pop().toLowerCase(); vb = b.is_dir ? 'dir' : b.name.split('.').pop().toLowerCase(); }
        else if (fmSortKey === 'mode') { va = a.mode || ''; vb = b.mode || ''; }
        else if (fmSortKey === 'mod_time') { va = a.mod_time; vb = b.mod_time; }
        else { va = a.name.toLowerCase(); vb = b.name.toLowerCase(); }

        if (va < vb) return fmSortAsc ? -1 : 1;
        if (va > vb) return fmSortAsc ?  1 : -1;
        return 0;
    });
}

export function renderFileRows(data) {
    const tbody = document.getElementById('files-table-body');
    if (!tbody) return;
    tbody.innerHTML = '';
    const sorted = sortData(data);
    sorted.forEach(item => {
        const tr = document.createElement('tr');
        tr.className = 'clickable-row';

        let iconClass = 'fa-solid fa-file file';
        if (item.is_dir) {
            iconClass = 'fa-solid fa-folder folder';
        } else if (item.name.endsWith('.zl') || item.name.endsWith('.html') || item.name.endsWith('.json') || item.name.endsWith('.go') || item.name.endsWith('.css') || item.name.endsWith('.js')) {
            iconClass = 'fa-solid fa-file-code code';
        } else if (item.name.endsWith('.zip') || item.name.endsWith('.tar') || item.name.endsWith('.gz') || item.name.endsWith('.7z')) {
            iconClass = 'fa-solid fa-file-zipper zip';
        }

        const sizeText = item.is_dir ? '-' : formatBytes(item.size);
        const itemPath = currentFilePath === '.' ? item.name :
                         currentFilePath === '/' ? '/' + item.name :
                         currentFilePath + '/' + item.name;
        const isZip = item.name.endsWith('.zip') || item.name.endsWith('.tar.gz') || item.name.endsWith('.7z');
        const typeText = item.is_dir ? 'Directory' : (item.name.includes('.') ? item.name.split('.').pop().toUpperCase() : 'File');
        const permText = formatPermissions(item.mode);

        tr.innerHTML = `
            <td style="width:36px;" onclick="event.stopPropagation()">
                <input type="checkbox" class="fm-row-check" data-path="${itemPath}" onchange="onRowCheckChange()">
            </td>
            <td>
                <i class="${iconClass} file-icon"></i>
                <span>${item.name}</span>
            </td>
            <td>${sizeText}</td>
            <td>${typeText}</td>
            <td style="font-family:var(--font-code); font-size:0.85rem;">${permText}</td>
            <td>${new Date(item.mod_time).toLocaleString()}</td>
            <td style="text-align:right;">
                ${!item.is_dir ? `<button class="btn-icon" style="color:var(--accent-primary)" onclick="editFile('${itemPath}')" title="Edit"><i class="fa-solid fa-pen-to-square"></i></button>` : ''}
                <button class="btn-icon" style="color:var(--success)" onclick="archiveFile('${itemPath}')" title="Compress to ZIP"><i class="fa-solid fa-file-zipper"></i></button>
                ${isZip ? `<button class="btn-icon" style="color:var(--warning)" onclick="extractFile('${itemPath}')" title="Extract Archive"><i class="fa-solid fa-folder-open"></i></button>` : ''}
                <button class="btn-icon" style="color:var(--text-muted)" onclick="changePermissionsPrompt('${itemPath}', '${item.mode}')" title="Ubah Permission"><i class="fa-solid fa-shield-halved"></i></button>
                <button class="btn-icon" style="color:var(--danger)" onclick="deleteFile('${itemPath}')" title="Delete"><i class="fa-solid fa-trash-can"></i></button>
            </td>
        `;

        tr.onclick = (e) => {
            if (e.target.type === 'checkbox') return;
            if (e.target.tagName !== 'BUTTON' && e.target.parentElement.tagName !== 'BUTTON' && e.target.tagName !== 'I') {
                if (item.is_dir) loadFilesList(itemPath);
                else editFile(itemPath);
            }
        };

        tbody.appendChild(tr);
    });
}

export function updateBulkBar() {
    const checked = document.querySelectorAll('.fm-row-check:checked');
    const bar = document.getElementById('fm-bulk-bar');
    const count = document.getElementById('fm-bulk-count');
    if (bar && count) {
        if (checked.length > 0) {
            bar.style.display = 'flex';
            count.textContent = checked.length + ' dipilih';
        } else {
            bar.style.display = 'none';
        }
    }
    const all = document.querySelectorAll('.fm-row-check');
    const selectAll = document.getElementById('fm-select-all');
    if (selectAll) selectAll.indeterminate = checked.length > 0 && checked.length < all.length;
    if (selectAll) selectAll.checked = all.length > 0 && checked.length === all.length;
}

export function onRowCheckChange() {
    updateBulkBar();
}

export function toggleSelectAll(checked) {
    document.querySelectorAll('.fm-row-check').forEach(cb => cb.checked = checked);
    updateBulkBar();
}

export function clearSelection() {
    document.querySelectorAll('.fm-row-check').forEach(cb => cb.checked = false);
    const selectAll = document.getElementById('fm-select-all');
    if (selectAll) { selectAll.checked = false; selectAll.indeterminate = false; }
    updateBulkBar();
}

export function getSelectedPaths() {
    return Array.from(document.querySelectorAll('.fm-row-check:checked')).map(cb => cb.dataset.path);
}

export function bulkDelete() {
    const paths = getSelectedPaths();
    if (paths.length === 0) return;
    if (!confirm(`Hapus ${paths.length} item yang dipilih? Tindakan ini tidak dapat dibatalkan.`)) return;
    Promise.all(paths.map(path =>
        fetch('/api/files/delete', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
            body: JSON.stringify({ path })
        }).then(r => r.json())
    )).then(results => {
        const failed = results.filter(r => !r.success).length;
        if (failed > 0) {
            showToast('error', `${failed} item gagal dihapus`);
        } else {
            showToast('success', `${paths.length} item berhasil dihapus`);
        }
        loadFilesList(currentFilePath);
    }).catch(err => showToast('error', 'Gagal menghapus: ' + err.toString()));
}

export function bulkArchive() {
    const paths = getSelectedPaths();
    if (paths.length === 0) return;
    let defaultName = paths.length === 1 ? paths[0].split('/').pop() + '.zip' : 'archive.zip';
    let zipName = prompt('Masukkan nama file ZIP untuk item yang dipilih:', defaultName);
    if (!zipName) return;
    const dest = currentFilePath === '.' ? zipName : currentFilePath + '/' + zipName;
    const promises = paths.length === 1
        ? [fetch('/api/files/archive', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
            body: JSON.stringify({ path: paths[0], dest })
        }).then(r => r.json())]
        : paths.map((path, i) => {
            const parts = dest.split('.');
            const ext = parts.pop();
            const destI = parts.join('.') + '_' + (i + 1) + '.' + ext;
            return fetch('/api/files/archive', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
                body: JSON.stringify({ path, dest: destI })
            }).then(r => r.json());
        });
    Promise.all(promises).then(results => {
        showToast('success', `${results.length} archive berhasil dibuat`);
        loadFilesList(currentFilePath);
    }).catch(err => showToast('error', 'Gagal mengarsipkan: ' + err.toString()));
}

export function loadFilesList(path) {
    currentFilePath = path;
    
    clearSelection();

    // Update path input
    const pathInput = document.getElementById('fm-path-input');
    if (pathInput) pathInput.value = path;

    // Build Breadcrumb UI
    const bc = document.getElementById('file-breadcrumb');
    if (bc) {
        bc.innerHTML = '';

        const isAbsolute = path.startsWith('/');
        const parts = path.split('/').filter(x => x && x !== '.');

        // Root / CWD anchor
        let rootSpan = document.createElement('span');
        if (isAbsolute) {
            rootSpan.innerText = '/';
            rootSpan.onclick = () => loadFilesList('/');
        } else {
            rootSpan.innerText = 'CWD';
            rootSpan.onclick = () => loadFilesList('.');
        }
        bc.appendChild(rootSpan);

        // Build cumulative path for each segment
        let cumPath = isAbsolute ? '' : '.';
        parts.forEach((p) => {
            cumPath = isAbsolute ? (cumPath + '/' + p) : (cumPath + '/' + p);
            const pathTarget = cumPath;

            const sep = document.createElement('span');
            sep.className = 'fm-breadcrumb-separator';
            sep.innerText = '>';
            bc.appendChild(sep);

            const span = document.createElement('span');
            span.innerText = p;
            span.onclick = () => loadFilesList(pathTarget);
            bc.appendChild(span);
        });
    }

    // Fetch directories
    fetch('/api/files/list?path=' + encodeURIComponent(path))
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                fmCurrentData = res.data;       // cache for sort
                updateSortHeaders();            // apply current sort indicator
                renderFileRows(fmCurrentData);  // render with current sort
            }
        })
        .catch(err => showToast('error', 'Gagal memuat file: ' + err.toString()));
}

export function goUpDirectory() {
    const p = currentFilePath;
    // Already at filesystem root
    if (p === '/' || p === '') {
        return;
    }
    // At CWD root (relative paths)
    if (p === '.' || p === '') {
        loadFilesList('.');
        return;
    }
    // Absolute path: go one level up
    if (p.startsWith('/')) {
        const parts = p.replace(/\/+$/, '').split('/');
        parts.pop();
        const parent = parts.join('/') || '/';
        loadFilesList(parent);
        return;
    }
    // Relative path: go one level up, fallback to CWD
    const parts = p.split('/');
    parts.pop();
    loadFilesList(parts.length > 0 ? parts.join('/') : '.');
}

export function createFilePrompt() {
    const name = prompt('Masukkan nama file baru:');
    if (name) {
        const fullPath = currentFilePath === '.' ? name : currentFilePath + '/' + name;
        fetch('/api/files/create-file', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': getCSRFToken()
            },
            body: JSON.stringify({ path: fullPath })
        })
        .then(res => res.json())
        .then(res => {
            if (res.success) {
                showToast('success', res.message);
                loadFilesList(currentFilePath);
            } else {
                showToast('error', res.message);
            }
        })
        .catch(err => showToast('error', 'Gagal membuat file: ' + err.toString()));
    }
}

export function createFolderPrompt() {
    const name = prompt('Masukkan nama folder baru:');
    if (name) {
        const fullPath = currentFilePath === '.' ? name : currentFilePath + '/' + name;
        fetch('/api/files/create-dir', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': getCSRFToken()
            },
            body: JSON.stringify({ path: fullPath })
        })
        .then(res => res.json())
        .then(res => {
            if (res.success) {
                showToast('success', res.message);
                loadFilesList(currentFilePath);
            } else {
                showToast('error', res.message);
            }
        })
        .catch(err => showToast('error', 'Gagal membuat folder: ' + err.toString()));
    }
}

export function deleteFile(path) {
    if (confirm(`Apakah Anda yakin ingin menghapus '${path}'?`)) {
        fetch('/api/files/delete', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': getCSRFToken()
            },
            body: JSON.stringify({ path: path })
        })
        .then(res => res.json())
        .then(res => {
            if (res.success) {
                showToast('success', res.message);
                loadFilesList(currentFilePath);
            } else {
                showToast('error', res.message);
            }
        });
    }
}

// Compress file
export function archiveFile(path) {
    let defaultZipName = path.split('/').pop() + '.zip';
    let zipName = prompt("Masukkan nama file ZIP tujuan kompresi:", defaultZipName);
    if (zipName) {
        let dest = currentFilePath === '.' ? zipName : currentFilePath + '/' + zipName;
        
        fetch('/api/files/archive', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': getCSRFToken()
            },
            body: JSON.stringify({ path: path, dest: dest })
        })
        .then(res => res.json())
        .then(res => {
            if (res.success) {
                showToast('success', res.message || 'Archive berhasil dibuat');
                loadFilesList(currentFilePath);
            } else {
                showToast('error', res.message || 'Gagal membuat archive');
            }
        })
        .catch(err => showToast('error', 'Gagal memanggil API: ' + err.toString()));
    }
}

// Extract ZIP
export function extractFile(path) {
    let defaultDest = currentFilePath;
    let dest = prompt("Masukkan folder tujuan ekstraksi:", defaultDest);
    if (dest) {
        fetch('/api/files/extract', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': getCSRFToken()
            },
            body: JSON.stringify({ path: path, dest: dest })
        })
        .then(res => res.json())
        .then(res => {
            if (res.success) {
                showToast('success', res.message || 'Archive berhasil diekstrak');
                loadFilesList(currentFilePath);
            } else {
                showToast('error', res.message || 'Gagal mengekstrak archive');
            }
        })
        .catch(err => showToast('error', 'Gagal memanggil API: ' + err.toString()));
    }
}

export function editFile(path) {
    activeEditorPath = path;
    const fnEl = document.getElementById('editor-modal-filename');
    if (fnEl) fnEl.innerText = 'Editing: ' + path;
    
    fetch('/api/files/read?path=' + encodeURIComponent(path))
        .then(res => res.json())
        .then(res => {
            if (res.success) {
                const taEl = document.getElementById('editor-textarea-field');
                if (taEl) taEl.value = res.content || '';
                const modal = document.getElementById('editor-modal');
                if (modal) modal.classList.add('active');
            } else {
                showToast('error', 'Gagal membaca isi file');
            }
        });
}

export function closeEditorModal() {
    const modal = document.getElementById('editor-modal');
    if (modal) modal.classList.remove('active');
    activeEditorPath = '';
}

export function saveActiveFile() {
    const taEl = document.getElementById('editor-textarea-field');
    const content = taEl ? taEl.value : '';
    fetch('/api/files/write', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ path: activeEditorPath, content: content })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message);
            closeEditorModal();
            loadFilesList(currentFilePath);
        } else {
            showToast('error', res.message);
        }
    });
}

/**
 * Membuka dialog pemilihan file bawaan browser saat tombol "Upload File" diklik
 */
export function triggerFileUpload() {
    const fileInput = document.getElementById('file-upload-input');
    if (fileInput) {
        fileInput.value = ''; // Reset input agar file yang sama bisa di-upload ulang jika perlu
        fileInput.click();
    }
}

/**
 * Menangani event ketika pengguna selesai memilih satu atau beberapa file
 */
export function handleFileUpload(event) {
    const files = event.target.files;
    if (!files || files.length === 0) return;

    // Siapkan objek FormData untuk mengirim data multipart
    const formData = new FormData();
    
    // Kirim lokasi folder aktif saat ini
    formData.append('path', currentFilePath);

    // Append semua file yang dipilih ke dalam form data
    for (let i = 0; i < files.length; i++) {
        formData.append('files', files[i]);
    }

    // Tampilkan notifikasi
    showToast('info', `Mengunggah ${files.length} file...`);

    // Lakukan pengiriman data ke backend menggunakan Fetch API
    fetch('/api/files/upload', {
        method: 'POST',
        headers: {
            // Header untuk bypass proteksi CSRF di backend
            'X-CSRF-Token': getCSRFToken()
        },
        body: formData
    })
    .then(res => {
        if (!res.ok) {
            return res.text().then(text => { throw new Error(text || 'Gagal mengunggah file') });
        }
        return res.json();
    })
    .then(res => {
        if (res.success) {
            showToast('success', res.message || 'File berhasil diunggah!');
            loadFilesList(currentFilePath);
        } else {
            showToast('error', res.message || 'Gagal mengunggah file.');
        }
    })
    .catch(err => {
        console.error("Upload error:", err);
        showToast('error', 'Terjadi kesalahan: ' + err.message);
    });
}

// Bind path input event
export function initFileManager() {
    const selectAll = document.getElementById('fm-select-all');
    if (selectAll) {
        selectAll.addEventListener('change', (e) => {
            toggleSelectAll(e.target.checked);
        });
    }

    const fileUploadInput = document.getElementById('file-upload-input');
    if (fileUploadInput) {
        fileUploadInput.addEventListener('change', (e) => {
            handleFileUpload(e);
        });
    }
}

// ---- File Permission Helpers ----

export function formatPermissions(modeStr) {
    if (!modeStr) return '-';
    const mode = parseInt(modeStr, 8);
    if (isNaN(mode)) return modeStr;

    // Get last 3 octal digits
    const octal = (mode & 0o777).toString(8).padStart(3, '0');

    let symbolic = '';
    // Owner
    symbolic += (mode & 0o400) ? 'r' : '-';
    symbolic += (mode & 0o200) ? 'w' : '-';
    symbolic += (mode & 0o100) ? 'x' : '-';

    // Group
    symbolic += (mode & 0o040) ? 'r' : '-';
    symbolic += (mode & 0o020) ? 'w' : '-';
    symbolic += (mode & 0o010) ? 'x' : '-';

    // Others
    symbolic += (mode & 0o004) ? 'r' : '-';
    symbolic += (mode & 0o002) ? 'w' : '-';
    symbolic += (mode & 0o001) ? 'x' : '-';

    return `${octal} (${symbolic})`;
}

export function changePermissionsPrompt(path, currentMode) {
    const modal = document.getElementById('permissions-modal');
    if (!modal) return;

    document.getElementById('perm-path-val').value = path;
    document.getElementById('perm-path-display').value = path;

    // Show/hide recursive checkbox depending on whether it's a directory
    const recursiveContainer = document.getElementById('perm-recursive-container');
    const isDir = fmCurrentData.find(item => {
        const itemPath = currentFilePath === '.' ? item.name :
                         currentFilePath === '/' ? '/' + item.name :
                         currentFilePath + '/' + item.name;
        return itemPath === path;
    })?.is_dir || false;

    if (recursiveContainer) {
        recursiveContainer.style.display = isDir ? 'block' : 'none';
    }

    const recursiveCb = document.getElementById('perm-recursive');
    if (recursiveCb) recursiveCb.checked = false;

    let initialMode = '644';
    if (currentMode) {
        const mode = parseInt(currentMode, 8);
        if (!isNaN(mode)) {
            initialMode = (mode & 0o777).toString(8).padStart(3, '0');
        }
    }

    const octalInput = document.getElementById('perm-octal-val');
    if (octalInput) {
        octalInput.value = initialMode;
    }
    updateCheckboxesFromOctal();

    modal.classList.add('active');
}

export function closePermissionsModal() {
    const modal = document.getElementById('permissions-modal');
    if (modal) modal.classList.remove('active');
}

export function updateOctalFromCheckboxes() {
    let owner = 0;
    if (document.getElementById('perm-owner-r').checked) owner += 4;
    if (document.getElementById('perm-owner-w').checked) owner += 2;
    if (document.getElementById('perm-owner-x').checked) owner += 1;

    let group = 0;
    if (document.getElementById('perm-group-r').checked) group += 4;
    if (document.getElementById('perm-group-w').checked) group += 2;
    if (document.getElementById('perm-group-x').checked) group += 1;

    let others = 0;
    if (document.getElementById('perm-others-r').checked) others += 4;
    if (document.getElementById('perm-others-w').checked) others += 2;
    if (document.getElementById('perm-others-x').checked) others += 1;

    const octalInput = document.getElementById('perm-octal-val');
    if (octalInput) {
        octalInput.value = `${owner}${group}${others}`;
    }
}

export function updateCheckboxesFromOctal() {
    const octalInput = document.getElementById('perm-octal-val');
    if (!octalInput) return;
    let val = octalInput.value.trim();
    if (val.length > 4) {
        val = val.substring(val.length - 4);
    }
    val = val.replace(/[^0-7]/g, '');
    octalInput.value = val;

    if (val.length === 3 || val.length === 4) {
        const last3 = val.substring(val.length - 3);
        const owner = parseInt(last3[0], 10);
        const group = parseInt(last3[1], 10);
        const others = parseInt(last3[2], 10);

        document.getElementById('perm-owner-r').checked = !!(owner & 4);
        document.getElementById('perm-owner-w').checked = !!(owner & 2);
        document.getElementById('perm-owner-x').checked = !!(owner & 1);

        document.getElementById('perm-group-r').checked = !!(group & 4);
        document.getElementById('perm-group-w').checked = !!(group & 2);
        document.getElementById('perm-group-x').checked = !!(group & 1);

        document.getElementById('perm-others-r').checked = !!(others & 4);
        document.getElementById('perm-others-w').checked = !!(others & 2);
        document.getElementById('perm-others-x').checked = !!(others & 1);
    }
}

export function submitChangePermissions() {
    const path = document.getElementById('perm-path-val').value;
    const mode = document.getElementById('perm-octal-val').value;
    const recursiveCb = document.getElementById('perm-recursive');
    const recursive = recursiveCb ? recursiveCb.checked : false;

    if (!mode || mode.length < 3) {
        showToast('error', 'Masukkan notasi oktal yang valid (contoh: 755)');
        return;
    }

    fetch('/api/files/chmod', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCSRFToken()
        },
        body: JSON.stringify({ path, mode, recursive })
    })
    .then(res => res.json())
    .then(res => {
        if (res.success) {
            showToast('success', res.message || 'Permission berhasil diperbarui');
            closePermissionsModal();
            loadFilesList(currentFilePath);
        } else {
            showToast('error', res.message || 'Gagal mengubah permission');
        }
    })
    .catch(err => showToast('error', 'Terjadi kesalahan: ' + err.toString()));
}
