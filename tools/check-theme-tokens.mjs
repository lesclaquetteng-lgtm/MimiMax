// Checks that the design system has no ghost token.
//
// Every `var(--name)` written in the interface must resolve to a definition, otherwise the property
// silently falls back to nothing: the rule is dropped and the element keeps an inherited value. The
// script also reports definitions that no rule uses, and the same token defined in two files (the
// second one wins, which is how a theme slowly stops being applied).
//
// Usage: node tools/check-theme-tokens.mjs [--quiet]

import { readFile, readdir } from 'node:fs/promises';
import { join, relative } from 'node:path';

const UI = 'launcher/gui/src/ui';
const quiet = process.argv.includes('--quiet');

const files = [];
for (const entry of await readdir(UI, { withFileTypes: true })) {
	if (entry.isFile() && /\.(css|html|js)$/.test(entry.name)) files.push(join(UI, entry.name));
}

/** Every `--name: value` declaration, with the file it comes from. */
const definitions = new Map(); // name -> [{ file, line, value }]
/** Every `var(--name)` reference, with the file it comes from. */
const usages = new Map(); // name -> [{ file, line }]

function linesOf(text) {
	return text.split(/\r?\n/);
}

for (const file of files) {
	const text = await readFile(file, 'utf8');
	const lines = linesOf(text);
	const short = relative(process.cwd(), file).replace(/\\/g, '/');

	// A declaration only counts inside a rule block: `--x: 1` in a comment is not a token.
	lines.forEach((line, index) => {
		const withoutComment = line.replace(/\/\*.*?\*\//g, '');
		for (const match of withoutComment.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/gi)) {
			const name = match[1];
			const value = match[2].trim();
			if (!definitions.has(name)) definitions.set(name, []);
			definitions.get(name).push({ file: short, line: index + 1, value });
		}

		for (const match of withoutComment.matchAll(/var\(\s*(--[a-z0-9-]+)/gi)) {
			const name = match[1];
			if (!usages.has(name)) usages.set(name, []);
			usages.get(name).push({ file: short, line: index + 1 });
		}
	});
}

const missing = [...usages.keys()].filter((name) => !definitions.has(name)).sort();
const unused = [...definitions.keys()].filter((name) => !usages.has(name)).sort();
const duplicated = [...definitions.entries()]
	.filter(([, places]) => new Set(places.map((place) => place.file)).size > 1)
	.sort(([a], [b]) => a.localeCompare(b));

const defined = definitions.size;
const referenced = usages.size;

if (!quiet) {
	for (const name of missing) {
		const where = usages.get(name).map((place) => `${place.file}:${place.line}`).join(', ');
		console.log(`MISSING  ${name}  used by ${where}`);
	}
	for (const [name, places] of duplicated) {
		const where = places.map((place) => `${place.file}:${place.line}`).join(', ');
		console.log(`DUPLICATE ${name}  defined in ${where}`);
	}
	if (unused.length > 0) {
		console.log(`UNUSED   ${unused.join(', ')}`);
	}
}

const resolved = referenced - missing.length;
console.log(
	`tokens: ${defined} defined, ${referenced} referenced, ${resolved}/${referenced} resolved (${(
		(resolved / referenced) *
		100
	).toFixed(1)}%), ${missing.length} missing, ${duplicated.length} duplicated, ${unused.length} unused`,
);

process.exit(missing.length === 0 && duplicated.length === 0 ? 0 : 1);
