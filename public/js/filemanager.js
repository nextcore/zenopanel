import { getCSRFToken, formatBytes, escapeHtml } from './utils.js';
import { showToast } from './toast.js';

export let currentFilePath = '/var/www';
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
        const isZip = item.name.endsWith('.zip') || item.name.endsWith('.tar.gz') || item.name.endsWith('.tgz') || item.name.endsWith('.7z');
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
                ${!item.is_dir ? `<button class="btn-icon" style="color:var(--info,#38bdf8)" onclick="downloadFile('${itemPath}')" title="Download"><i class="fa-solid fa-cloud-arrow-down"></i></button>` : ''}
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
            count.textContent = checked.length + ' selected';
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
    if (!confirm(`Delete the ${paths.length} selected item(s)? This action cannot be undone.`)) return;
    Promise.all(paths.map(path =>
        fetch('/api/files/delete', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
            body: JSON.stringify({ path })
        }).then(r => r.json())
    )).then(results => {
        const failed = results.filter(r => !r.success).length;
        if (failed > 0) {
            showToast('error', `Failed to delete ${failed} item(s)`);
        } else {
            showToast('success', `${paths.length} item(s) deleted successfully`);
        }
        loadFilesList(currentFilePath);
    }).catch(err => showToast('error', 'Failed to delete: ' + err.toString()));
}

export function bulkArchive() {
    const paths = getSelectedPaths();
    if (paths.length === 0) return;
    let defaultName = paths.length === 1 ? (paths[0].split('/').pop().includes('.') ? paths[0].split('/').pop().split('.').slice(0, -1).join('.') : paths[0].split('/').pop()) + '.zip' : 'archive.zip';
    let zipName = prompt('Enter archive name for selected items (supports .zip, .tar.gz):', defaultName);
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
        showToast('success', `${results.length} archive(s) created successfully`);
        loadFilesList(currentFilePath);
    }).catch(err => showToast('error', 'Failed to archive: ' + err.toString()));
}

export function loadFilesList(path) {
    currentFilePath = path;
    
    clearSelection();
    updatePasteButton();

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
        .catch(err => showToast('error', 'Failed to load files: ' + err.toString()));
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
    const name = prompt('Enter new file name:');
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
        .catch(err => showToast('error', 'Failed to create file: ' + err.toString()));
    }
}

export function createFolderPrompt() {
    const name = prompt('Enter new folder name:');
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
        .catch(err => showToast('error', 'Failed to create folder: ' + err.toString()));
    }
}

export function deleteFile(path) {
    if (confirm(`Are you sure you want to delete '${path}'?`)) {
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

// Download file — opens the binary download endpoint in a hidden <a> tag
export function downloadFile(path) {
    const url = '/api/files/download?path=' + encodeURIComponent(path);
    const a = document.createElement('a');
    a.href = url;
    a.download = path.split('/').pop();
    a.style.display = 'none';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
}

// Compress file
export function archiveFile(path) {
    let baseName = path.split('/').pop();
    let defaultZipName = (baseName.includes('.') ? baseName.split('.').slice(0, -1).join('.') : baseName) + '.zip';
    let zipName = prompt("Enter target archive name (supports .zip, .tar.gz):", defaultZipName);
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
                showToast('success', res.message || 'Archive created successfully');
                loadFilesList(currentFilePath);
            } else {
                showToast('error', res.message || 'Failed to create archive');
            }
        })
        .catch(err => showToast('error', 'Failed to call API: ' + err.toString()));
    }
}

// Extract ZIP
export function extractFile(path) {
    let defaultDest = currentFilePath;
    let dest = prompt("Enter destination folder for extraction:", defaultDest);
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
                showToast('success', res.message || 'Archive extracted successfully');
                loadFilesList(currentFilePath);
            } else {
                showToast('error', res.message || 'Failed to extract archive');
            }
        })
        .catch(err => showToast('error', 'Failed to call API: ' + err.toString()));
    }
}

let monacoInstance = null;

function getOrInitMonaco(container, callback) {
    if (window.monaco) {
        callback();
        return;
    }

    const loaderScript = document.createElement('script');
    loaderScript.src = '/public/js/monaco/vs/loader.js';
    loaderScript.onload = () => {
        require.config({ paths: { vs: '/public/js/monaco/vs' } });
        require(['vs/editor/editor.main'], () => {
            callback();
        });
    };
    document.head.appendChild(loaderScript);
}

export function editFile(path) {
    activeEditorPath = path;
    const fnEl = document.getElementById('editor-modal-filename');
    if (fnEl) fnEl.innerText = 'Editing: ' + path;
    
    // Detect language from file extension
    const ext = path.split('.').pop().toLowerCase();
    let language = 'plaintext';
    if (ext === 'zl' || ext === 'html') language = 'html';
    else if (ext === 'js' || ext === 'mjs') language = 'javascript';
    else if (ext === 'css') language = 'css';
    else if (ext === 'json') language = 'json';
    else if (ext === 'sh' || ext === 'bash') language = 'shell';
    else if (ext === 'go') language = 'go';
    else if (ext === 'rs') language = 'rust';
    else if (ext === 'py') language = 'python';
    else if (ext === 'md') language = 'markdown';
    else if (ext === 'yml' || ext === 'yaml') language = 'yaml';

    fetch('/api/files/read?path=' + encodeURIComponent(path))
        .then(res => {
            if (!res.ok) {
                throw new Error('HTTP ' + res.status);
            }
            return res.json();
        })
        .then(res => {
            if (res.success) {
                const taEl = document.getElementById('editor-textarea-field');
                if (taEl) taEl.value = res.content || '';
                
                const container = document.getElementById('monaco-editor-container');
                if (container) {
                    getOrInitMonaco(container, () => {
                        if (!monacoInstance) {
                            monacoInstance = monaco.editor.create(container, {
                                value: res.content || '',
                                language: language,
                                theme: 'vs-dark',
                                automaticLayout: true,
                                fontSize: 13,
                                fontFamily: 'var(--font-code)',
                                minimap: { enabled: true }
                            });
                        } else {
                            monacoInstance.setValue(res.content || '');
                            monaco.editor.setModelLanguage(monacoInstance.getModel(), language);
                        }
                    });
                }
                
                const modal = document.getElementById('editor-modal');
                if (modal) modal.classList.add('active');
            } else {
                showToast('error', 'Failed to read file content');
            }
        })
        .catch(err => {
            showToast('warning', 'File not found. Creating a new script file...');
            
            // Auto-create directory and file with template content
            fetch('/api/files/write', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'X-CSRF-Token': getCSRFToken()
                },
                body: JSON.stringify({
                    path: path,
                    content: '#!/bin/bash\n\n# Write your script commands below\n'
                })
            })
            .then(wRes => wRes.json())
            .then(wRes => {
                if (wRes.success) {
                    const templateContent = '#!/bin/bash\n\n# Write your script commands below\n';
                    const taEl = document.getElementById('editor-textarea-field');
                    if (taEl) taEl.value = templateContent;
                    
                    const container = document.getElementById('monaco-editor-container');
                    if (container) {
                        getOrInitMonaco(container, () => {
                            if (!monacoInstance) {
                                monacoInstance = monaco.editor.create(container, {
                                    value: templateContent,
                                    language: 'shell',
                                    theme: 'vs-dark',
                                    automaticLayout: true,
                                    fontSize: 13,
                                    fontFamily: 'var(--font-code)',
                                    minimap: { enabled: true }
                                });
                            } else {
                                monacoInstance.setValue(templateContent);
                                monaco.editor.setModelLanguage(monacoInstance.getModel(), 'shell');
                            }
                        });
                    }
                    
                    const modal = document.getElementById('editor-modal');
                    if (modal) modal.classList.add('active');
                } else {
                    showToast('error', 'Failed to create new script file');
                }
            })
            .catch(wErr => {
                showToast('error', 'Failed to create file: ' + wErr.message);
            });
        });
}

export function closeEditorModal() {
    const modal = document.getElementById('editor-modal');
    if (modal) modal.classList.remove('active');
    activeEditorPath = '';
    
    // Dispose of Monaco editor instance to avoid DOM conflicts with HTMX swaps
    if (monacoInstance) {
        monacoInstance.dispose();
        monacoInstance = null;
    }
}

export function saveActiveFile() {
    let content = '';
    if (monacoInstance) {
        content = monacoInstance.getValue();
    } else {
        const taEl = document.getElementById('editor-textarea-field');
        content = taEl ? taEl.value : '';
    }
    
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
    showToast('info', `Uploading ${files.length} file(s)...`);

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
            return res.text().then(text => { throw new Error(text || 'Failed to upload files') });
        }
        return res.json();
    })
    .then(res => {
        if (res.success) {
            showToast('success', res.message || 'File(s) uploaded successfully!');
            loadFilesList(currentFilePath);
        } else {
            showToast('error', res.message || 'Failed to upload files.');
        }
    })
    .catch(err => {
        console.error("Upload error:", err);
        showToast('error', 'An error occurred: ' + err.message);
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

    // Drag & Drop Upload
    const tableContainer = document.querySelector('#tab-files .table-container');
    if (tableContainer) {
        tableContainer.addEventListener('dragover', (e) => {
            e.preventDefault();
            tableContainer.style.background = 'rgba(59, 130, 246, 0.08)';
            tableContainer.style.border = '2px dashed var(--accent-primary)';
        });
        
        tableContainer.addEventListener('dragleave', (e) => {
            e.preventDefault();
            tableContainer.style.background = 'transparent';
            tableContainer.style.border = 'none';
        });
        
        tableContainer.addEventListener('drop', (e) => {
            e.preventDefault();
            tableContainer.style.background = 'transparent';
            tableContainer.style.border = 'none';
            
            const files = e.dataTransfer.files;
            if (files && files.length > 0) {
                const uploadInput = document.getElementById('file-upload-input');
                if (uploadInput) {
                    uploadInput.files = files;
                    const event = { target: { files: files } };
                    handleFileUpload(event);
                }
            }
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
        showToast('error', 'Enter a valid octal notation (e.g., 755)');
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
            showToast('success', res.message || 'Permissions updated successfully');
            closePermissionsModal();
            loadFilesList(currentFilePath);
        } else {
            showToast('error', res.message || 'Failed to update permissions');
        }
    })
    .catch(err => showToast('error', 'An error occurred: ' + err.toString()));
}

// Clipboard state for Copy/Cut/Paste
export let fmClipboard = null;

export function updatePasteButton() {
    const btn = document.getElementById('fm-btn-paste');
    const countSpan = document.getElementById('fm-paste-count');
    if (btn && countSpan) {
        if (fmClipboard && fmClipboard.items && fmClipboard.items.length > 0) {
            btn.style.display = 'inline-flex';
            countSpan.textContent = fmClipboard.items.length;
        } else {
            btn.style.display = 'none';
        }
    }
}

export function bulkCopy() {
    const paths = getSelectedPaths();
    if (paths.length === 0) {
        showToast('warning', 'Please select items to copy first');
        return;
    }
    fmClipboard = { type: 'copy', items: paths };
    showToast('success', `${paths.length} item(s) copied to clipboard`);
    clearSelection();
    updatePasteButton();
}

export function bulkCut() {
    const paths = getSelectedPaths();
    if (paths.length === 0) {
        showToast('warning', 'Please select items to cut first');
        return;
    }
    fmClipboard = { type: 'cut', items: paths };
    showToast('success', `${paths.length} item(s) cut to clipboard`);
    clearSelection();
    updatePasteButton();
}

export function pasteClipboard() {
    if (!fmClipboard || !fmClipboard.items || fmClipboard.items.length === 0) {
        showToast('warning', 'Clipboard is empty');
        return;
    }
    const { type, items } = fmClipboard;
    const promises = items.map(src => {
        const name = src.split('/').pop();
        const dest = currentFilePath === '.' ? name : currentFilePath + '/' + name;
        const url = type === 'copy' ? '/api/files/copy' : '/api/files/move';
        return fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': getCSRFToken() },
            body: JSON.stringify({ src, dest })
        }).then(r => r.json());
    });

    showToast('info', `Processing ${items.length} item(s)...`);

    Promise.all(promises).then(results => {
        const failed = results.filter(r => !r.success).length;
        if (failed > 0) {
            showToast('error', `Failed to process ${failed} of ${items.length} item(s)`);
        } else {
            showToast('success', `Successfully ${type === 'copy' ? 'copied' : 'moved'} ${items.length} item(s)`);
        }
        if (type === 'cut') {
            fmClipboard = null; // Clear clipboard for cut items
        }
        updatePasteButton();
        loadFilesList(currentFilePath);
    }).catch(err => {
        showToast('error', 'Failed to paste: ' + err.toString());
    });
}

export function filterFilesList(query) {
    const q = query.toLowerCase().trim();
    if (!q) {
        renderFileRows(fmCurrentData);
        return;
    }
    const filtered = fmCurrentData.filter(item => item.name.toLowerCase().includes(q));
    renderFileRows(filtered);
}

window.filterFilesList = filterFilesList;
window.goUpDirectory = goUpDirectory;
window.createFilePrompt = createFilePrompt;
window.createFolderPrompt = createFolderPrompt;
window.triggerFileUpload = triggerFileUpload;
window.handleFileUpload = handleFileUpload;
window.loadFilesList = loadFilesList;
window.setSortBy = setSortBy;
window.toggleSelectAll = toggleSelectAll;
window.onRowCheckChange = onRowCheckChange;
window.bulkCopy = bulkCopy;
window.bulkCut = bulkCut;
window.bulkDelete = bulkDelete;
window.bulkArchive = bulkArchive;
window.clearSelection = clearSelection;
window.pasteClipboard = pasteClipboard;
window.editFile = editFile;
window.downloadFile = downloadFile;
window.archiveFile = archiveFile;
window.extractFile = extractFile;
window.changePermissionsPrompt = changePermissionsPrompt;
window.closePermissionsModal = closePermissionsModal;
window.submitChangePermissions = submitChangePermissions;
window.updateOctalFromCheckboxes = updateOctalFromCheckboxes;
window.updateCheckboxesFromOctal = updateCheckboxesFromOctal;
window.saveActiveFile = saveActiveFile;
window.closeEditorModal = closeEditorModal;

export function openTerminalHere() {
    window.pendingTerminalCwd = currentFilePath;
    if (window.switchTab) {
        window.switchTab("terminal");
    } else {
        const btn = document.querySelector(`.nav-item[data-tab="terminal"]`);
        if (btn) btn.click();
    }
}

window.openTerminalHere = openTerminalHere;
