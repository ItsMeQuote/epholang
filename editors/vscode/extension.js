const vscode = require('vscode');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function runEphoFMT(source, executablePath) {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'ephofmt-'));
    const filePath = path.join(directory, 'document.epho');

    try {
        fs.writeFileSync(filePath, source, 'utf8');

        const result = spawnSync(executablePath, ['format', filePath], {
            encoding: 'utf8',
            windowsHide: true,
            timeout: 15000,
            maxBuffer: 10 * 1024 * 1024
        });

        if (result.error) {
            throw new Error('Could not start Epholang CLI "' + executablePath + '": ' + result.error.message);
        }

        if (result.status !== 0) {
            const details = (result.stderr || result.stdout || '').trim();
            throw new Error(details || ('Epholang CLI exited with code ' + result.status));
        }

        return fs.readFileSync(filePath, 'utf8');
    } finally {
        try {
            fs.rmSync(directory, { recursive: true, force: true });
        } catch {
            // Temporary-file cleanup must not hide the formatter result.
        }
    }
}

function minimalEdit(document, formatted) {
    const original = document.getText();
    if (original === formatted) {
        return [];
    }

    let prefix = 0;
    const maxPrefix = Math.min(original.length, formatted.length);
    while (prefix < maxPrefix && original[prefix] === formatted[prefix]) {
        prefix += 1;
    }

    let suffix = 0;
    const maxSuffix = Math.min(original.length - prefix, formatted.length - prefix);
    while (
        suffix < maxSuffix &&
        original[original.length - suffix - 1] === formatted[formatted.length - suffix - 1]
    ) {
        suffix += 1;
    }

    const range = new vscode.Range(
        document.positionAt(prefix),
        document.positionAt(original.length - suffix)
    );

    return [
        vscode.TextEdit.replace(
            range,
            formatted.slice(prefix, formatted.length - suffix)
        )
    ];
}

function activate(context) {
    const formatter = vscode.languages.registerDocumentFormattingEditProvider(
        { language: 'epholang' },
        {
            provideDocumentFormattingEdits(document) {
                const executablePath = vscode.workspace
                    .getConfiguration('epholang')
                    .get('executablePath', 'epholang');

                try {
                    const formatted = runEphoFMT(document.getText(), executablePath);
                    return minimalEdit(document, formatted);
                } catch (error) {
                    vscode.window.showErrorMessage('EphoFMT: ' + error.message);
                    return [];
                }
            }
        }
    );

    context.subscriptions.push(formatter);
}

function deactivate() {}

module.exports = { activate, deactivate };
