import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { runInNewContext } from 'node:vm';
import { fileURLToPath } from 'node:url';

const base = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => JSON.parse(fs.readFileSync(path.join(base, p), 'utf8'));
const pkg = read('package.json');
const lang = read('language-configuration.json');
const grammar = read('syntaxes/epholang.tmLanguage.json');
const snippets = read('snippets/epholang.json');
assert.equal(pkg.version, '0.2.0');
assert.equal(pkg.contributes.languages[0].id, 'epholang');
assert.deepEqual(pkg.contributes.languages[0].extensions, ['.epho']);
assert.equal(pkg.contributes.grammars[0].scopeName, grammar.scopeName);
assert(pkg.contributes.snippets.some(s=>s.language==='epholang'));
assert(lang.brackets.length >= 3);
assert.deepEqual(lang.comments, { blockComment: ['/*', '*/'] });
assert.equal(lang.folding.offSide, true);
assert.match('if ready:', new RegExp(lang.onEnterRules[0].beforeText));
const patterns = grammar.repository.tokens.patterns;
for (const p of patterns) assert.doesNotThrow(() => new RegExp(p.match), `Bad regex: ${p.name}`);
const find = (scope) => patterns.find(p => p.name === scope);
const match = (scope, input) => new RegExp(find(scope).match).exec(input);
assert(match('meta.function.declaration.epholang', 'function CalculateArea(flt radius) -> flt:'));
assert(match('meta.function.declaration.epholang', 'function calculateArea(flt radius) -> flt:')); // style is advisory
assert(!match('meta.function.declaration.epholang', 'CalculateArea func := function(flt radius) -> flt:'));
assert.equal(match('meta.function-call.module.epholang','math:RandomInterval(1, 6)')[3], 'RandomInterval');
assert.equal(match('meta.function-call.module.epholang','stringTools:ToUpperCase("hi")')[1], 'stringTools');
for (const token of ['int', 'flt', 'str', 'bool', 'none', 'type', 'ls', 'list']) {
  assert(match('storage.type.epholang', token), `Not highlighted: ${token}`);
}
for (const func of ['Input', 'Output', 'Includes', 'Length', 'Insert', 'Remove']) {
  assert(match('support.function.builtin.epholang', `${func}(`), `Not highlighted: ${func}`);
}
assert(match('constant.numeric.float.epholang','3.1415'));
assert(match('constant.numeric.integer.epholang','9223372036854775807'));
assert(match('invalid.illegal.operator.epholang','1 >= 0'));
assert.equal(grammar.repository.comments.begin, '/\\*');
assert(grammar.repository.comments.patterns.some(p=>p.include==='#comments'));
assert(grammar.repository.strings.patterns.some(p=>p.include==='#interpolation'));
assert(snippets.Function.body.join('\n').includes('function'));
assert(snippets.Function.body.join('\n').includes('return'));
assert(snippets['Import module'].body.join('\n').includes('import'));

assert(match('keyword.control.epholang', 'loop'));
assert(match('keyword.control.epholang', 'else'));
assert(!match('keyword.control.epholang', 'otherwise'));
assert(match('punctuation.section.block.epholang', 'if ready:'));
assert(!match('punctuation.section.block.epholang', 'math:Sum('));
assert(match('punctuation.separator.return-type.epholang', '->'));
assert(match('punctuation.separator.return-type.epholang', '-|'));
assert(match('keyword.operator.discard.epholang', '_ = 42'));
assert(!match('keyword.operator.discard.epholang', '_ := 42'));
assert(!match('variable.other.readwrite.epholang', '_'));
assert(match('variable.other.readwrite.epholang', '_value'));
assert(snippets['Function without a result'].body[0].endsWith('-|:'));
assert(snippets['Discard result'].body[0].startsWith('_ = '));
assert.equal(snippets.Function.body[1][0], '\t');
assert.equal(snippets['If / else'].body[2], 'else:');
assert(!match('keyword.control.epholang', 'cycle'));
assert(snippets['Loop with condition'].body.join('\n').includes('loop'));
assert(snippets['Loop over list'].body.join('\n').includes('loop'));

assert.equal(pkg.main, './extension.js');
assert(pkg.activationEvents.includes('onLanguage:epholang'));
assert.equal(pkg.contributes.configurationDefaults['[epholang]']['editor.formatOnSave'], true);
assert.equal(pkg.contributes.configurationDefaults['[epholang]']['editor.insertSpaces'], false);
assert.equal(pkg.contributes.configuration.properties['epholang.executablePath'].default, 'epholang');
assert(fs.existsSync(path.join(base, pkg.main)));

const { spawnSync } = await import('node:child_process');
const syntaxCheck = spawnSync(process.execPath, ['--check', path.join(base, pkg.main)], { encoding: 'utf8' });
assert.equal(syntaxCheck.status, 0, syntaxCheck.stderr);

const executable = process.env.EPHOLANG_BIN;
if (executable) {
  const entry = path.join(base, pkg.main);
  const require = createRequire(entry);
  const extension = { exports: {} };
  const errors = [];
  const uri = { scheme: 'file', path: '/examples/test.epho' };
  let provider;
  const vscode = {
    languages: {
      registerDocumentFormattingEditProvider(selector, registered) {
        assert.equal(selector.language, 'epholang');
        provider = registered;
        return { dispose() {} };
      },
    },
    workspace: {
      getConfiguration(namespace, scope) {
        assert.equal(namespace, 'epholang');
        assert.equal(scope, uri);
        return { get: () => path.resolve(executable) };
      },
    },
    window: { showErrorMessage: message => errors.push(message) },
    Range: class { constructor(start, end) { this.start = start; this.end = end; } },
    TextEdit: { replace: (range, text) => ({ range, text }) },
  };
  runInNewContext(fs.readFileSync(entry, 'utf8'), {
    require: name => name === 'vscode' ? vscode : require(name),
    module: extension,
    AbortController,
  });
  extension.exports.activate({ subscriptions: [] });
  const token = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose() {} }) };

  function document(source) {
    return {
      uri, version: 1, getText: () => source,
      positionAt(offset) {
        assert.notEqual(source.slice(offset - 1, offset + 1), '\r\n');
        return offset;
      },
    };
  }
  const source = 'function F()-| :\r\n\tif true :\r\n\t\tOutput("🙂");_=42\r\nF()\r\n';
  const formatted = 'function F() -|:\n\tif true:\n\t\tOutput("🙂")\n\t\t_ = 42\nF()\n';
  const edits = await provider.provideDocumentFormattingEdits(document(source), {}, token);
  const [{ range, text }] = edits;
  assert.equal(source.slice(0, range.start) + text + source.slice(range.end), formatted);
  assert.equal((await provider.provideDocumentFormattingEdits(document(formatted), {}, token)).length, 0);

  const bad = await provider.provideDocumentFormattingEdits(document('if true:\n  Output(1)'), {}, token);
  assert.equal(bad.length, 0);
  assert.match(errors.pop(), /Indentation must use tabs/);

  const changing = document(source);
  const pending = provider.provideDocumentFormattingEdits(changing, {}, token);
  changing.version += 1;
  assert.equal((await pending).length, 0);

  const cancelled = await provider.provideDocumentFormattingEdits(
    document(source), {}, { ...token, isCancellationRequested: true },
  );
  assert.equal(cancelled.length, 0);
  assert.equal(errors.length, 0);
}

console.log(executable
  ? 'Extension manifest, grammar, snippets and real EphoFMT process: OK'
  : 'Extension manifest, grammar, snippets and JavaScript syntax: OK');
