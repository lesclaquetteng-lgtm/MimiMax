'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const {
	createOperationGate,
	isSameAppDocument,
	isTrustedIpcEvent,
	normalizeExternalUrl
} = require('../gui/src/security.cjs');

test('external links accept HTTPS only', () => {
	assert.equal(normalizeExternalUrl('https://example.com/path?q=1'), 'https://example.com/path?q=1');
	assert.equal(normalizeExternalUrl('http://example.com'), null);
	assert.equal(normalizeExternalUrl('javascript:alert(1)'), null);
	assert.equal(normalizeExternalUrl('file:///C:/Windows/win.ini'), null);
	assert.equal(normalizeExternalUrl('data:text/html,hello'), null);
});

test('external links reject malformed, oversized and credential-bearing URLs', () => {
	assert.equal(normalizeExternalUrl('not a URL'), null);
	assert.equal(normalizeExternalUrl('https://user:pass@example.com'), null);
	assert.equal(normalizeExternalUrl(`https://example.com/${'a'.repeat(2048)}`), null);
	assert.equal(normalizeExternalUrl(null), null);
});

test('navigation stays in the same local application document', () => {
	const current = 'file:///C:/Program%20Files/Aether/ui/index.html';
	assert.equal(isSameAppDocument('file:///C:/Program%20Files/Aether/ui/index.html#settings', current), true);
	assert.equal(isSameAppDocument('file:///C:/Program%20Files/Aether/ui/other.html', current), false);
	assert.equal(isSameAppDocument('https://example.com/', current), false);
	assert.equal(isSameAppDocument('not a URL', current), false);
});

test('IPC is accepted only from the top-level frame of the main window', () => {
	const mainFrame = {};
	const sender = { mainFrame };
	const window = { webContents: sender };

	assert.equal(isTrustedIpcEvent({ sender, senderFrame: mainFrame }, window), true);
	assert.equal(isTrustedIpcEvent({ sender, senderFrame: {} }, window), false);
	assert.equal(isTrustedIpcEvent({ sender: {}, senderFrame: mainFrame }, window), false);
	assert.equal(isTrustedIpcEvent({ sender, senderFrame: null }, window), false);
});

test('operation gate rejects overlapping work and releases after completion', async () => {
	const gate = createOperationGate();
	let release;
	const first = gate.run('install', () => new Promise((resolve) => { release = resolve; }));

	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(gate.isBusy('install'), true);
	assert.deepEqual(await gate.run('install', async () => 'should not run'), {
		ok: false,
		busy: true,
		error: 'Another install operation is already running.'
	});

	release('complete');
	assert.equal(await first, 'complete');
	assert.equal(gate.isBusy('install'), false);
	assert.equal(await gate.run('install', async () => 'next run'), 'next run');
});

test('operation gate releases its lock when work throws', async () => {
	const gate = createOperationGate();
	await assert.rejects(gate.run('game', async () => { throw new Error('boom'); }), /boom/);
	assert.equal(gate.isBusy('game'), false);
});
