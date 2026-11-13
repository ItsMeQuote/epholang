import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {codeToTokens} from 'shiki';
const grammar={...JSON.parse(readFileSync('../editors/vscode/syntaxes/epholang.tmLanguage.json','utf8')),name:'epholang'};
test('Epholang nested comments and interpolation retain their source and token boundaries',async()=>{
 const source=String.raw`/* outer /* inner */ still comment */
Output("Value: #{types:Convert(\"42\", int)}\n")`;
 const result=await codeToTokens(source,{lang:grammar,theme:'github-dark'});
 assert.equal(result.tokens.map(line=>line.map(token=>token.content).join('')).join('\n'),source);
 assert.equal(new Set(result.tokens[0].map(token=>token.color)).size,1,'nested comment remains one comment scope');
 assert(result.tokens[1].some(token=>token.content.includes('Output')));
 assert(new Set(result.tokens[1].map(token=>token.color)).size>=4,'interpolation receives distinct code tokens');
});
