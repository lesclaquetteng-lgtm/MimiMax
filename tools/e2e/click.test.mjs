// Guards the guard: every e2e check that presses a button must go through tools/e2e/click.mjs,
// which checks the point with `document.elementFromPoint` before sending any mouse event.
//
// These tests are meant to bite: neutralising the hit test inside `click.mjs` has to turn them red
// (see ANALYSE-AMELIORATIONS.md, « La mutation du garde »). That is why they pin the decision line
// itself, not only the presence of the call.
//
// Usage: node --test tools/e2e/click.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const source = (path) => readFileSync(join(here, path), 'utf8');
const scripts = readdirSync(here).filter((name) => name.endsWith('.mjs') && !name.endsWith('.test.mjs'));

// The header comment speaks about elementFromPoint too: the order of the *code* is what matters, so
// the comments are removed before it is measured.
const code = (path) => source(path).replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');

// The checks that click or drag a control of the launcher. This list is the count the documentation
// has to give: the test below compares the two.
const CLICKING = ['a11y.mjs', 'cancel.mjs', 'crash-report.mjs', 'failure.mjs', 'hit.mjs', 'profiles.mjs', 'sliders.mjs', 'stubborn.mjs'];

test('the shared helper asks elementFromPoint before it presses', () => {
	const click = code('click.mjs');
	assert.match(click, /document\.elementFromPoint\(x, y\)/);
	assert.ok(
		click.indexOf('elementFromPoint') < click.indexOf("Input.dispatchMouseEvent"),
		'the hit test has to run before the mouse event is built'
	);
	assert.match(click, /is covered by/, 'a miss has to say what is in the way');
	assert.match(click, /is outside the window/);
});

test('the reachable decision comes from the hit test, not from a constant', () => {
	const click = code('click.mjs');
	assert.match(click, /const at = onScreen \? document\.elementFromPoint\(x, y\) : null;/, 'the point is not looked up');
	assert.match(
		click,
		/const reachable = !!at && \(at === element \|\| element\.contains\(at\)\);/,
		'the hit test has to decide what is reachable'
	);
	assert.doesNotMatch(click, /reachable\s*=\s*true/, 'a constant would accept every click');
	assert.match(click, /problem: reachable\s*\n?\s*\?\s*''/, 'the refusal has to follow the decision');
});

test('every checking script imports the guarded helper', () => {
	for (const name of CLICKING) {
		assert.match(source(name), /from '\.\/click\.mjs'/, `${name} does not use tools/e2e/click.mjs`);
	}
});

test('no script but the helper and the slider drag sends a press itself', () => {
	// sliders.mjs keeps its own press/release pair for the drag; its start point goes through
	// pointAt(), which is the same hit test.
	const pressing = scripts.filter((name) => name !== 'click.mjs' && /'mousePressed'/.test(source(name)));
	assert.deepEqual(pressing, ['sliders.mjs']);

	const sliders = source('sliders.mjs');
	const checkDrag = sliders.slice(sliders.indexOf('async function checkDrag'));
	assert.match(checkDrag, /pointAt\(evaluate, selector, 0\.1\)/, 'the drag start point is not checked');
	assert.match(checkDrag, /await drag\(start\.box/, 'the drag has to start from the checked point');
});

test('no script presses a hardcoded coordinate', () => {
	for (const name of scripts) {
		assert.doesNotMatch(
			source(name),
			/'mousePressed'[\s\S]{0,200}?x:\s*\d/,
			`${name} sends a press to a coordinate written in the source`
		);
	}
});

test('no check swallows a refusal around a click', () => {
	for (const name of scripts) {
		const text = source(name);
		for (const block of text.matchAll(/try\s*\{([\s\S]*?)\}\s*catch/g)) {
			assert.doesNotMatch(block[1], /clickElement\(|await\s+click\(/, `${name} wraps a click in a try block`);
		}
	}

	// The one place that catches a click is hit.mjs, whose helper must fail when the click was
	// accepted instead of returning quietly.
	const hit = source('hit.mjs');
	assert.doesNotMatch(hit, /function attempt/, 'a generic try/catch would let a refusal pass');
	assert.match(hit, /if \(!message\) throw new Error/, 'an accepted click has to fail the check');
	assert.match(hit, /const covered = await expectRefusal\(/, 'the covered case goes through expectRefusal');
});

test('the proof of the guard runs with the suite', () => {
	assert.match(source('replay.ps1'), /script = 'hit\.mjs'/, 'hit.mjs is not part of replay.ps1');
	assert.match(source('hit.mjs'), /is covered by #probe-overlay/);
	assert.match(source('hit.mjs'), /under a second/);
});

test('the documentation gives the number of guarded scripts the code has', () => {
	const french = ['zéro', 'un', 'deux', 'trois', 'quatre', 'cinq', 'six', 'sept', 'huit', 'neuf', 'dix'];
	const english = ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten'];
	const forms = [french[CLICKING.length], english[CLICKING.length]];
	const docs = ['../../README.md', '../../ANALYSE-AMELIORATIONS.md', '../../docs/VERIFICATION.md'];

	for (const doc of docs) {
		const text = source(doc);
		assert.ok(
			forms.some((word) => new RegExp(`\\b${word}\\s+scripts?\\b`, 'i').test(text)),
			`${doc} has to say "${forms[0]} scripts" (the code guards ${CLICKING.length})`
		);
	}
});
