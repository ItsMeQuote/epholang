import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
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
const patterns = grammar.repository.tokens.patterns;
for (const p of patterns) assert.doesNotThrow(() => new RegExp(p.match), `Bad regex: ${p.name}`);
const find = (scope) => patterns.find(p => p.name === scope);
const match = (scope, input) => new RegExp(find(scope).match).exec(input);
assert(match('meta.function.declaration.epholang', 'function CalculateArea(radius flt) flt {'));
assert(match('meta.function.declaration.epholang', 'function calculateArea(radius flt) flt {')); // style is advisory
assert(!match('meta.function.declaration.epholang', 'CalculateArea func := function(radius flt) flt {'));
assert.equal(match('meta.function-call.module.epholang','math:RandomInterval(1, 6)')[3], 'RandomInterval');
assert.equal(match('meta.function-call.module.epholang','stringTools:ToUpperCase("hi")')[1], 'stringTools');
for (const token of ['int', 'flt', 'str', 'bool', 'none', 'type', 'ls', 'list']) {
  assert(match('storage.type.epholang', token), `Not highlighted: ${token}`);
}
for (const func of ['Input', 'Output', 'Includes', 'Length', 'Insert', 'Remove', 'TypeOf']) {
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
assert(!match('keyword.control.epholang', 'cycle'));
assert(snippets['Loop with condition'].body.join('\n').includes('loop'));
assert(snippets['Loop over list'].body.join('\n').includes('loop'));
console.log('Extension manifest, grammar patterns, comments, interpolation and snippets: OK');
