import { getCSRFToken } from './utils.js';
import { showToast } from './toast.js';

export let allUsers = [];

export async function loadUsers() {
    try {
        const response = await fetch('/api/users/list');
        if (!response.ok) {
            if (response.status === 403) return; // Silent if not authorized
            throw new Error('Failed to fetch users');
        }
        allUsers = await response.json();
        renderUsers();
    } catch (err) {
        showToast('Error loading users: ' + err.message, 'error');
    }
}

export function renderUsers() {
    const tbody = document.getElementById('users-table-body');
    if (!tbody) return;

    if (allUsers.length === 0) {
        tbody.innerHTML = `<tr><td colspan="5" style="text-align:center; color:var(--text-muted); padding:20px;">No users found</td></tr>`;
        return;
    }

    tbody.innerHTML = allUsers.map(user => {
        let roleBadge = '';
        if (user.role === 'admin') {
            roleBadge = `<span style="background:rgba(239,68,68,0.1); color:#ef4444; border:1px solid rgba(239,68,68,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem; font-weight:600;">ADMIN</span>`;
        } else if (user.role === 'editor') {
            roleBadge = `<span style="background:rgba(245,158,11,0.1); color:#f59e0b; border:1px solid rgba(245,158,11,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem; font-weight:600;">EDITOR</span>`;
        } else {
            roleBadge = `<span style="background:rgba(16,185,129,0.1); color:#10b981; border:1px solid rgba(16,185,129,0.2); padding:3px 8px; border-radius:4px; font-size:0.75rem; font-weight:600;">VIEWER</span>`;
        }

        return `
            <tr>
                <td>${user.id}</td>
                <td><strong>${user.username}</strong></td>
                <td>${roleBadge}</td>
                <td>${user.created_at}</td>
                <td style="text-align:right;">
                    <div style="display:inline-flex; gap:8px; justify-content:flex-end;">
                        <button class="btn-action btn-sm" onclick="openEditUserModal('${user.username}', '${user.role}')" style="padding:4px 8px; font-size:0.75rem; color:var(--warning); border-color:rgba(245,158,11,0.2); background:rgba(245,158,11,0.05);">
                            <i class="fa-solid fa-edit"></i> Edit
                        </button>
                        <button class="btn-action btn-sm" onclick="deleteUser('${user.username}')" style="padding:4px 8px; font-size:0.75rem; color:#ef4444; border-color:transparent; background:transparent;">
                            <i class="fa-solid fa-trash-can"></i> Delete
                        </button>
                    </div>
                </td>
            </tr>
        `;
    }).join('');
}

function resetPasswordVisibility() {
    const pwdInput = document.getElementById('user-password');
    const toggleIcon = document.getElementById('toggle-user-password-icon');
    if (pwdInput && toggleIcon) {
        pwdInput.type = 'password';
        pwdInput.placeholder = '••••••••';
        toggleIcon.classList.remove('fa-eye-slash');
        toggleIcon.classList.add('fa-eye');
    }
}

export function toggleUserPasswordVisibility() {
    const pwdInput = document.getElementById('user-password');
    const toggleIcon = document.getElementById('toggle-user-password-icon');
    const mode = document.getElementById('user-mode').value;
    if (pwdInput && toggleIcon) {
        if (pwdInput.type === 'password') {
            pwdInput.type = 'text';
            toggleIcon.classList.remove('fa-eye');
            toggleIcon.classList.add('fa-eye-slash');
            if (pwdInput.value === '') {
                pwdInput.placeholder = mode === 'edit' ? '(leave blank to keep current)' : 'Enter password';
            }
        } else {
            pwdInput.type = 'password';
            toggleIcon.classList.remove('fa-eye-slash');
            toggleIcon.classList.add('fa-eye');
            pwdInput.placeholder = '••••••••';
        }
    }
}

export function openAddUserModal() {
    document.getElementById('modal-user-title').innerText = 'Add User';
    document.getElementById('user-mode').value = 'create';
    
    const usernameInput = document.getElementById('user-username');
    usernameInput.value = '';
    usernameInput.disabled = false;
    
    document.getElementById('user-password').value = '';
    resetPasswordVisibility();
    document.getElementById('pwd-help-text').innerText = '';
    document.getElementById('user-role').value = 'admin';
    
    document.getElementById('add-user-modal').classList.add('active');
}

export function openEditUserModal(username, role) {
    document.getElementById('modal-user-title').innerText = 'Edit User';
    document.getElementById('user-mode').value = 'edit';
    
    const usernameInput = document.getElementById('user-username');
    usernameInput.value = username;
    usernameInput.disabled = true;
    
    document.getElementById('user-password').value = '';
    resetPasswordVisibility();
    document.getElementById('pwd-help-text').innerText = ' (leave blank to keep current)';
    document.getElementById('user-role').value = role;
    
    document.getElementById('add-user-modal').classList.add('active');
}

export function closeAddUserModal() {
    resetPasswordVisibility();
    document.getElementById('add-user-modal').classList.remove('active');
}

export async function submitAddUser() {
    const mode = document.getElementById('user-mode').value;
    const username = document.getElementById('user-username').value.trim();
    const password = document.getElementById('user-password').value;
    const role = document.getElementById('user-role').value;

    if (!username) {
        showToast('Username is required', 'error');
        return;
    }

    if (mode === 'create' && !password) {
        showToast('Password is required', 'error');
        return;
    }

    const csrfToken = getCSRFToken();

    try {
        let url = '/api/users/create';
        let payload = {
            username: username,
            password_plain: password,
            role: role
        };

        if (mode === 'edit') {
            url = '/api/users/update';
            payload = {
                username: username,
                role: role,
                password_plain: password || null
            };
        }

        const response = await fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify(payload)
        });

        const data = await response.json();
        if (data.success) {
            showToast(data.message || 'User saved successfully', 'success');
            closeAddUserModal();
            loadUsers();
        } else {
            showToast(data.message || 'Failed to save user', 'error');
        }
    } catch (err) {
        showToast('Error saving user: ' + err.message, 'error');
    }
}

export async function deleteUser(username) {
    if (!confirm(`Are you sure you want to delete user "${username}"?`)) {
        return;
    }

    const csrfToken = getCSRFToken();

    try {
        const response = await fetch('/api/users/delete', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrfToken
            },
            body: JSON.stringify({ username: username })
        });

        const data = await response.json();
        if (data.success) {
            showToast(data.message || 'User deleted successfully', 'success');
            loadUsers();
        } else {
            showToast(data.message || 'Failed to delete user', 'error');
        }
    } catch (err) {
        showToast('Error deleting user: ' + err.message, 'error');
    }
}
