const vscode = require('vscode');

function activate(context) {
    const diagnostics = vscode.languages.createDiagnosticCollection('xcx');
    context.subscriptions.push(diagnostics);

    if (vscode.window.activeTextEditor) {
        validateDocument(vscode.window.activeTextEditor.document, diagnostics);
    }

    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument(e => validateDocument(e.document, diagnostics))
    );
    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(ed => { if (ed) validateDocument(ed.document, diagnostics); })
    );
    context.subscriptions.push(
        vscode.workspace.onDidCloseTextDocument(doc => diagnostics.delete(doc.uri))
    );
}

function deactivate() { }

function validateDocument(document, diagnostics) {
    if (document.languageId !== 'xcx') return;
    const errors = [];
    const lines = document.getText().split('\n');
    for (let i = 0; i < lines.length; i++) {
        const raw = lines[i];
        const line = stripComment(raw);
        const trimmed = line.trim();
        if (trimmed === '') continue;
        checkMissingSemicolon(trimmed, raw, i, errors);
        checkIfThen(trimmed, raw, i, errors);
        checkWhileDo(trimmed, raw, i, errors);
        checkForDo(trimmed, raw, i, errors);
        checkFuncFiberBrace(trimmed, raw, i, errors);
        checkEndSemicolon(trimmed, raw, i, errors);
        checkElseSemicolon(trimmed, raw, i, errors);
        checkUnclosedRawBlock(trimmed, raw, i, errors);
    }
    checkRawBlockBalance(lines, errors);
    diagnostics.set(document.uri, errors);
}

// ── helpers ──────────────────────────────────────────────────

function stripComment(line) {
    let inStr = false;
    for (let i = 0; i < line.length - 2; i++) {
        if (line[i] === '"') inStr = !inStr;
        if (!inStr && line[i] === '-' && line[i + 1] === '-' && line[i + 2] === '-') {
            return line.substring(0, i);
        }
    }
    return line;
}

function makeDiag(lineIdx, colStart, colEnd, message, severity) {
    const range = new vscode.Range(
        new vscode.Position(lineIdx, colStart),
        new vscode.Position(lineIdx, colEnd)
    );
    return new vscode.Diagnostic(range, message, severity ?? vscode.DiagnosticSeverity.Error);
}

function endCol(raw) { return raw.trimEnd().length; }

// ── checks ────────────────────────────────────────────────────

function checkMissingSemicolon(trimmed, raw, i, errors) {
    const skip = [
        /^\{$/, /\{$/, /^end[;\s]/, /^end$/, /^\};/, /^\}$/,
        /^---/, /^<<</, /^>>>/, /^serve:/, /^table:/, /^map:/,
        /^columns\s*=/, /^rows\s*=/, /^schema\s*=/, /^data\s*=/,
        /^port\s*=/, /^host\s*=/, /^workers\s*=/, /^routes\s*=/,
        /^\[/, /^\]$/, /,$/, /^".*::/, /^\*/,
        /^elseif\b/, /^elif\b/, /^elf\b/, /^else[;\s]/, /^els[;\s]/,
        /^else$/, /^els$/,
    ];
    for (const p of skip) if (p.test(trimmed)) return;

    const needs = [
        /^(i|f|s|b|json|date|array:[a-z]+|set:[NZQSBC]|map|table|fiber(:[a-zA-Z]+)?)\s*:/,
        /^const\s/, /^>!/, /^>\?/, /^@wait\s/,
        /^halt\.(alert|error|fatal)/,
        /^include\s/, /^yield\b/, /^return\b/,
        /^break$/, /^continue$/,
        /^\.[a-zA-Z]+\s*![a-z]+/,
        /^[a-zA-Z_][a-zA-Z0-9_.]*\s*=[^=]/,
        /^[a-zA-Z_][a-zA-Z0-9_.]*\(.*\)\s*$/,
        /^store\./, /^db\.[a-zA-Z]+\.(insert|update|delete)/,
        /^[a-zA-Z_][a-zA-Z0-9_.]*\.(insert|update|delete|push|set|bind)\(/,
    ];
    let should = false;
    for (const p of needs) if (p.test(trimmed)) { should = true; break; }
    if (!should) return;

    if (!trimmed.trimEnd().endsWith(';')) {
        const col = endCol(raw);
        errors.push(makeDiag(i, col, col + 1, "Brakujący średnik ';' na końcu instrukcji."));
    }
}

function checkIfThen(trimmed, raw, i, errors) {
    if (!/^(if|elif|elseif|elf)\s*\(/.test(trimmed)) return;
    if (!/\bthen\s*;/.test(trimmed)) {
        const col = endCol(raw);
        errors.push(makeDiag(i, col, col + 1, "Oczekiwane 'then;' po warunku if."));
    }
}

function checkWhileDo(trimmed, raw, i, errors) {
    if (!/^while\s*\(/.test(trimmed)) return;
    if (!/\bdo\s*;/.test(trimmed)) {
        const col = endCol(raw);
        errors.push(makeDiag(i, col, col + 1, "Oczekiwane 'do;' po warunku while."));
    }
}

function checkForDo(trimmed, raw, i, errors) {
    if (!/^for\s+/.test(trimmed)) return;
    if (!/\bdo\s*;/.test(trimmed)) {
        const col = endCol(raw);
        errors.push(makeDiag(i, col, col + 1, "Oczekiwane 'do;' w pętli for."));
    }
}

function checkFuncFiberBrace(trimmed, raw, i, errors) {
    if (!/^(func|fiber)\s/.test(trimmed)) return;
    if (!trimmed.includes('{')) {
        const col = endCol(raw);
        errors.push(makeDiag(i, col, col + 1, "Oczekiwane '{' po sygnaturze func/fiber."));
    }
}

function checkEndSemicolon(trimmed, raw, i, errors) {
    if (!/^end\b/.test(trimmed)) return;
    if (!trimmed.includes(';')) {
        const col = raw.indexOf('end');
        errors.push(makeDiag(i, col, col + 3, "'end' musi kończyć się średnikiem: 'end;'."));
    }
}

function checkElseSemicolon(trimmed, raw, i, errors) {
    if (!/^(else|els)\s*$/.test(trimmed)) return;
    const col = endCol(raw);
    errors.push(makeDiag(i, col, col + 1, "'else' musi kończyć się średnikiem: 'else;'."));
}

function checkUnclosedRawBlock(trimmed, raw, i, errors) {
    const opens = (trimmed.match(/<<</g) || []).length;
    const closes = (trimmed.match(/>>>/g) || []).length;
    if (opens > closes) {
        const col = raw.indexOf('<<<');
        errors.push(makeDiag(i, col, col + 3,
            "Niezamknięty blok JSON '<<<' — brakuje '>>>'.",
            vscode.DiagnosticSeverity.Warning));
    }
}

function checkRawBlockBalance(lines, errors) {
    let depth = 0, openLine = -1;
    for (let i = 0; i < lines.length; i++) {
        const line = stripComment(lines[i]);
        const opens = (line.match(/<<</g) || []).length;
        const closes = (line.match(/>>>/g) || []).length;
        if (opens > 0 && depth === 0) openLine = i;
        depth += opens - closes;
        if (depth < 0) {
            errors.push(makeDiag(i, 0, lines[i].length,
                "Nieoczekiwane '>>>' bez pasującego '<<<'."));
            depth = 0;
        }
    }
    if (depth > 0 && openLine >= 0) {
        errors.push(makeDiag(openLine, 0, lines[openLine].length,
            "Niezamknięty blok JSON '<<<' — brakuje '>>>' do końca pliku."));
    }
}

module.exports = { activate, deactivate };