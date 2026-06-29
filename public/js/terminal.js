import { getCSRFToken, escapeHtml } from './utils.js';

export const terminalHistory = [];
export let terminalHistoryIndex = -1;

export let terminalCwd = "";
let terminalUsername = "root";
let terminalHostname = "control-panel";
let terminalSessionId = "";
let terminalEventSource = null;

export function focusTerminalInput() {
    const input = document.getElementById('terminal-stdin');
    if (input) input.focus();
}

export function closeTerminal() {
    if (terminalEventSource) {
        terminalEventSource.close();
        terminalEventSource = null;
    }
}

function updatePrompt(cwd) {
    const promptNode = document.getElementById('terminal-prompt-node');
    if (!promptNode) return;
    
    const symbol = terminalUsername === 'root' ? '#' : '$';
    
    // Normalize home directory path to tilde (~)
    let displayDir = cwd;
    const homePath = terminalUsername === 'root' ? '/root' : `/home/${terminalUsername}`;
    if (displayDir === homePath) {
        displayDir = "~";
    } else if (displayDir.startsWith(homePath + "/")) {
        displayDir = "~" + displayDir.slice(homePath.length);
    }
    
    promptNode.innerText = `${terminalUsername}@${terminalHostname}:${displayDir}${symbol}`;
}

export function handleTerminalCommand(event) {
    if (event.key === 'Enter') {
        const inputField = document.getElementById('terminal-stdin');
        if (!inputField) return;
        const command = inputField.value.trim();
        inputField.value = '';

        if (!command) return;

        terminalHistory.push(command);
        terminalHistoryIndex = terminalHistory.length;

        // Print command to console output viewport
        const viewport = document.getElementById('terminal-viewport');
        if (!viewport) return;
        
        const promptNode = document.getElementById('terminal-prompt-node');
        const currentPrompt = promptNode ? promptNode.innerText : 'root@control-panel:~#';
        const promptLine = document.createElement('div');
        promptLine.className = 'terminal-line';
        promptLine.innerHTML = `<span class="terminal-prompt">${escapeHtml(currentPrompt)}</span> ${escapeHtml(command)}`;
        viewport.insertBefore(promptLine, inputField.parentElement);

        if (command === 'clear') {
            const lines = Array.from(viewport.querySelectorAll('.terminal-line'));
            const outputs = Array.from(viewport.querySelectorAll('.terminal-output'));
            lines.forEach(l => l.remove());
            outputs.forEach(o => o.remove());
            return;
        }

        // Send input to the persistent bash session's stdin
        // Append CWD tracking command dynamically
        const payloadInput = `${command}\necho -n "___CWD_DELIMITER___"; pwd\n`;
        
        fetch('/api/terminal/write', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': getCSRFToken()
            },
            body: JSON.stringify({
                session_id: terminalSessionId,
                input: payloadInput
            })
        })
        .catch(err => {
            const errLine = document.createElement('div');
            errLine.className = 'terminal-output terminal-error';
            errLine.innerText = 'Failed to send input: ' + err.toString();
            viewport.insertBefore(errLine, inputField.parentElement);
            viewport.scrollTop = viewport.scrollHeight;
        });
    } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        const inputField = document.getElementById('terminal-stdin');
        if (!inputField) return;
        if (terminalHistory.length > 0 && terminalHistoryIndex > 0) {
            terminalHistoryIndex--;
            inputField.value = terminalHistory[terminalHistoryIndex];
        }
    } else if (event.key === 'ArrowDown') {
        event.preventDefault();
        const inputField = document.getElementById('terminal-stdin');
        if (!inputField) return;
        if (terminalHistory.length > 0 && terminalHistoryIndex < terminalHistory.length - 1) {
            terminalHistoryIndex++;
            inputField.value = terminalHistory[terminalHistoryIndex];
        } else {
            terminalHistoryIndex = terminalHistory.length;
            inputField.value = '';
        }
    }
}

export function initTerminal() {
    const input = document.getElementById('terminal-stdin');
    if (input) {
        input.addEventListener('keydown', (e) => {
            handleTerminalCommand(e);
        });
    }

    // Load actual user and hostname to make the prompt dynamic
    fetch('/api/info')
        .then(res => res.json())
        .then(res => {
            if (res.success && res.data) {
                terminalUsername = res.data.username || 'max';
                terminalHostname = res.data.hostname || 'control-panel';
                
                // Initialize unique session ID and SSE connection
                terminalSessionId = Math.random().toString(36).substring(2) + Date.now().toString(36);
                
                if (terminalEventSource) {
                    terminalEventSource.close();
                }
                
                terminalEventSource = new EventSource(`/api/terminal/stream?session_id=${terminalSessionId}`);
                
                // Current output buffer/element
                let currentOutputElement = null;
                
                terminalEventSource.onmessage = (event) => {
                    const viewport = document.getElementById('terminal-viewport');
                    if (!viewport) return;
                    
                    let data = event.data;
                    
                    // Parse out dynamic CWD changes
                    if (data.includes("___CWD_DELIMITER___")) {
                        const parts = data.split("___CWD_DELIMITER___");
                        data = parts[0];
                        terminalCwd = parts[1].trim();
                        updatePrompt(terminalCwd);
                    }
                    
                    if (data !== "") {
                        if (!currentOutputElement) {
                            currentOutputElement = document.createElement('pre');
                            currentOutputElement.className = 'terminal-output';
                            currentOutputElement.style.cssText = "margin: 0; white-space: pre-wrap; font-family: var(--font-code); font-size: 0.85rem; color: #f1f5f9; line-height: 1.5; padding: 4px 0;";
                            viewport.insertBefore(currentOutputElement, input.parentElement);
                        }
                        
                        currentOutputElement.textContent += data;
                        viewport.scrollTop = viewport.scrollHeight;
                    } else {
                        // Reset output element for new command outputs
                        currentOutputElement = null;
                    }
                };
                
                terminalEventSource.onerror = () => {
                    console.warn("Terminal stream disconnected, attempting reconnect...");
                };
                
                // Trigger initial CWD retrieval
                setTimeout(() => {
                    fetch('/api/terminal/write', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                            'X-CSRF-Token': getCSRFToken()
                        },
                        body: JSON.stringify({
                            session_id: terminalSessionId,
                            input: 'echo -n "___CWD_DELIMITER___"; pwd\n'
                        })
                    }).catch(() => {});
                }, 500);
            }
        })
        .catch(err => console.error(err));
}
