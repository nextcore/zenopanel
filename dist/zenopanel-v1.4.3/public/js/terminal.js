export const terminalHistory = [];
export let terminalHistoryIndex = -1;
export let terminalCwd = "";

let socket = null;
let term = null;
let fitAddon = null;

export function focusTerminalInput() {
    if (term) term.focus();
}

export function handleTerminalCommand(event) {
    // Deprecated in xterm.js implementation
}

export function closeTerminal() {
    if (socket) {
        socket.close();
        socket = null;
    }
    if (term) {
        term.dispose();
        term = null;
    }
}

export function initTerminal() {
    const termContainer = document.getElementById('terminal');
    if (!termContainer) return;
    
    // Only initialize once
    if (term) return;

    term = new Terminal({
        cursorBlink: true,
        theme: {
            background: '#05070c',
            foreground: '#f1f5f9',
            cursor: '#f1f5f9',
            selectionBackground: 'rgba(255, 255, 255, 0.15)',
        },
        fontFamily: 'var(--font-code)',
        fontSize: 14,
    });
    
    fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);
    term.open(termContainer);
    
    // Perform initial fit
    setTimeout(() => {
        if (fitAddon) fitAddon.fit();
    }, 100);

    // Setup websocket connection
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    let wsUrl = `${protocol}//${window.location.host}/api/terminal/ws`;
    if (window.pendingContainerTerminalId) {
        wsUrl += `?container=${encodeURIComponent(window.pendingContainerTerminalId)}`;
        window.pendingContainerTerminalId = null; // reset
    }
    
    socket = new WebSocket(wsUrl);
    
    socket.onopen = () => {
        // Send initial terminal size
        const dims = fitAddon.proposeDimensions();
        if (dims) {
            socket.send(JSON.stringify({
                type: 'resize',
                cols: dims.cols,
                rows: dims.rows
            }));
        }
    };
    
    socket.onmessage = (event) => {
        term.write(event.data);
    };
    
    socket.onclose = () => {
        term.write('\r\n\x1b[31mTerminal connection closed.\x1b[0m\r\n');
    };
    
    socket.onerror = (err) => {
        console.error('Terminal websocket error:', err);
    };
    
    term.onData((data) => {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(data);
        }
    });

    // Handle resize
    const resizeObserver = new ResizeObserver(() => {
        if (fitAddon && term) {
            try {
                fitAddon.fit();
                const dims = fitAddon.proposeDimensions();
                if (dims && socket && socket.readyState === WebSocket.OPEN) {
                    socket.send(JSON.stringify({
                        type: 'resize',
                        cols: dims.cols,
                        rows: dims.rows
                    }));
                }
            } catch (e) {
                // Ignore layout errors during transitions
            }
        }
    });
    resizeObserver.observe(termContainer);
}
