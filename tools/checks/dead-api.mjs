// One-shot scan: public or protected methods of the mod that no source file calls.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = 'aether-client/src';
const files = [];
(function walk(dir) {
	for (const entry of readdirSync(dir)) {
		const path = join(dir, entry);
		if (statSync(path).isDirectory()) walk(path);
		else if (path.endsWith('.java')) files.push(path);
	}
})(ROOT);

const sources = files.map((file) => ({ file, text: readFileSync(file, 'utf8') }));
const declarations = [];
const SKIP = new Set(['if', 'for', 'while', 'switch', 'catch', 'return', 'new', 'case', 'record']);

for (const { file, text } of sources) {
	text.split('\n').forEach((line, index) => {
		const match = line.match(/^\s{1,2}(public|protected)\s+(static\s+)?(final\s+)?[\w<>[\], .?]+\s+([a-zA-Z_]\w*)\s*\(/);
		if (match && !SKIP.has(match[4])) {
			declarations.push({ file, line: index + 1, name: match[4], signature: line.trim().slice(0, 100) });
		}
	});
}

const dead = [];
for (const declaration of declarations) {
	const pattern = new RegExp('\\b' + declaration.name + '\\s*\\(', 'g');
	let total = 0;
	for (const { text } of sources) total += (text.match(pattern) || []).length;
	if (total <= 1) dead.push(declaration);
}

console.log(`declarations: ${declarations.length}, without caller: ${dead.length}`);
for (const entry of dead) console.log(`${entry.file.replace('aether-client/src/', '')}:${entry.line} | ${entry.signature}`);
