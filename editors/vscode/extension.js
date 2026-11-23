const vscode = require('vscode');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');

const execFileAsync = promisify(execFile);

async function runEphoFMT(source, executablePath, signal) {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'ephofmt-'));
    const filePath = path.join(directory, 'document.epho');

    try {
        fs.writeFileSync(filePath, source, 'utf8');

        await execFileAsync(executablePath, ['format', filePath], {
            encoding: 'utf8',
            signal,
            windowsHide: true,
            timeout: 15000,
            maxBuffer: 10 * 1024 * 1024
        });

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

    // A CRLF and a surrogate pair each occupy two UTF-16 code units but
    // cannot be represented by an edit boundary between those units.
    while (prefix > 0 && (splitsCharacter(original, prefix) || splitsCharacter(formatted, prefix))) {
        prefix -= 1;
    }
    while (suffix > 0 && (
        splitsCharacter(original, original.length - suffix) ||
        splitsCharacter(formatted, formatted.length - suffix)
    )) {
        suffix -= 1;
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

function splitsCharacter(text, offset) {
    const before = text.charCodeAt(offset - 1);
    const after = text.charCodeAt(offset);
    return (before === 13 && after === 10) ||
        (before >= 0xD800 && before <= 0xDBFF && after >= 0xDC00 && after <= 0xDFFF);
}

function activate(context) {
    const formatter = vscode.languages.registerDocumentFormattingEditProvider(
        { language: 'epholang' },
        {
            async provideDocumentFormattingEdits(document, _options, token) {
                if (token.isCancellationRequested) {
                    return [];
                }
                const executablePath = vscode.workspace
                    .getConfiguration('epholang', document.uri)
                    .get('executablePath', 'epholang');
                const version = document.version;
                const controller = new AbortController();
                const cancellation = token.onCancellationRequested(() => controller.abort());

                try {
                    const formatted = await runEphoFMT(document.getText(), executablePath, controller.signal);
                    if (token.isCancellationRequested || document.version !== version) {
                        return [];
                    }
                    return minimalEdit(document, formatted);
                } catch (error) {
                    if (!token.isCancellationRequested && document.version === version) {
                        vscode.window.showErrorMessage('EphoFMT: ' + (error.stderr?.trim() || error.message));
                    }
                    return [];
                } finally {
                    cancellation.dispose();
                }
            }
        }
    );

    context.subscriptions.push(formatter);
}

function deactivate() {}

module.exports = { activate, deactivate };
