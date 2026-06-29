// --- TOAST NOTIFICATIONS CODE ---
export function showToast(type, message) {
    const container = document.getElementById('toast-bin');
    if (!container) return;
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    
    const icon = type === 'success' ? 'fa-solid fa-circle-check' : (type === 'warning' ? 'fa-solid fa-triangle-exclamation' : 'fa-solid fa-circle-exclamation');
    toast.innerHTML = `<i class="${icon}"></i> <span>${message}</span>`;
    
    container.appendChild(toast);
    
    setTimeout(() => {
        toast.style.animation = 'slideIn 0.3s ease reverse forwards';
        setTimeout(() => toast.remove(), 300);
    }, 3500);
}
