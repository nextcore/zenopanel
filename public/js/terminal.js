import { getCSRFToken, escapeHtml } from './utils.js';

export const terminalHistory = [];
export let terminalHistoryIndex = -1;

export function focusTerminalInput() {
    const input = document.getElementById('terminal-stdin');
    if (input) input.focus();
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

        // Send request to execute command
        fetch('/api/terminal/run', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-CSRF-Token': getCSRFToken()
            },
            body: JSON.stringify({ command: command })
        })
        .then(res => res.json())
        .then(res => {
            const resultContainer = document.createElement('div');
            resultContainer.className = 'terminal-output';
            
            if (res.success) {
                const outText = res.stdout || 'Command completed with exit code 0';
                resultContainer.innerText = outText;
            } else {
                const errText = res.stderr || `Command failed with exit code ${res.exit_code}`;
                resultContainer.innerHTML = `<span class="terminal-error">${escapeHtml(errText)}</span>`;
            }
            viewport.insertBefore(resultContainer, inputField.parentElement);
            
            viewport.scrollTop = viewport.scrollHeight;
        })
        .catch(err => {
            const errLine = document.createElement('div');
            errLine.className = 'terminal-output terminal-error';
            errLine.innerText = 'Gagal memanggil shell execution: ' + err.toString();
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
                const username = res.data.username || 'max';
                const hostname = res.data.hostname || 'control-panel';
                // root uses '#', non-root uses '$'
                const symbol = username === 'root' ? '#' : '$';
                const promptString = `${username}@${hostname}:~${symbol}`;

                const promptNode = document.getElementById('terminal-prompt-node');
                if (promptNode) {
                    promptNode.innerText = promptString;
                }
            }
        })
        .catch(err => console.error(err));
}
