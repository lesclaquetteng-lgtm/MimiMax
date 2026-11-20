// Checks the keyboard and the screen reader contract of the launcher with real input events:
// the dialog announces itself, takes the focus, keeps it, closes on Escape and gives it back, and
// an error toast is announced.
//
// Usage: node a11y.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9222';
const results = [];

function record(name, ok, detail = '') {
	results.push({ name, ok, detail });
	console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
}

async function findPage() {
	for (let attempt = 0; attempt < 120; attempt++) {
		try {
			const answer = await fetch(`http://127.0.0.1:${port}/json/list`);
			const targets = await answer.json();
			const page = targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
			if (page) return page.webSocketDebuggerUrl;
		} catch {
			// still starting
		}
		await sleep(500);
	}
	throw new Error(`no debugging endpoint on port ${port}`);
}

const socket = new WebSocket(await findPage());
await new Promise((resolve, reject) => {
	socket.onopen = resolve;
	socket.onerror = () => reject(new Error('the renderer refused the connection'));
});

let nextId = 1;
const pending = new Map();

socket.onmessage = (event) => {
	const message = JSON.parse(event.data);
	if (message.id && pending.has(message.id)) {
		pending.get(message.id)(message);
		pending.delete(message.id);
	}
};

function send(method, params = {}) {
	const id = nextId++;
	socket.send(JSON.stringify({ id, method, params }));
	return new Promise((resolve) => pending.set(id, resolve));
}

async function evaluate(expression) {
	const answer = await send('Runtime.evaluate', {
		expression,
		awaitPromise: true,
		returnByValue: true,
		userGesture: true
	});
	const exception = answer.result?.exceptionDetails;
	if (exception) throw new Error(exception.exception?.description || exception.text);
	return answer.result?.result?.value;
}

async function waitFor(expression, description, timeout = 30000) {
	const start = Date.now();
	while (Date.now() - start < timeout) {
		if (await evaluate(expression)) return true;
		await sleep(200);
	}
	throw new Error(`timeout while waiting for ${description}`);
}

/** Clicks the centre of an element with the mouse, so the focus lands where a player would. */
async function clickElement(selector) {
	const box = await evaluate(`(() => {
		const rect = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();
		return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
	})()`);

	for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased']) {
		await send('Input.dispatchMouseEvent', {
			type,
			x: box.x,
			y: box.y,
			button: 'left',
			buttons: type === 'mousePressed' ? 1 : 0,
			clickCount: 1
		});
	}
	await sleep(200);
}

async function pressKey(key, virtualKeyCode) {
	await send('Input.dispatchKeyEvent', {
		type: key === 'Tab' ? 'rawKeyDown' : 'keyDown',
		key,
		code: key,
		windowsVirtualKeyCode: virtualKeyCode,
		nativeVirtualKeyCode: virtualKeyCode
	});
	await send('Input.dispatchKeyEvent', {
		type: 'keyUp',
		key,
		code: key,
		windowsVirtualKeyCode: virtualKeyCode,
		nativeVirtualKeyCode: virtualKeyCode
	});
	await sleep(200);
}

await send('Runtime.enable');
await waitFor(
	`document.querySelector('#paths-output') && document.querySelector('#paths-output').textContent.length > 0`,
	'the initial state'
);

// --- the dialog -------------------------------------------------------------------------
await evaluate(`document.querySelector('.nav-item[data-page="accounts"]').click(), true`);
await sleep(300);
await clickElement('#btn-add-offline');

const dialog = await evaluate(`(() => {
	const box = document.querySelector('.modal');
	return {
		open: !document.querySelector('#modal').hidden,
		role: box.getAttribute('role'),
		modal: box.getAttribute('aria-modal'),
		labelledBy: box.getAttribute('aria-labelledby'),
		titleId: document.querySelector('#modal-title').id,
		focus: document.activeElement.id
	};
})()`);

record('the dialog is announced as a modal dialog',
	dialog.open && dialog.role === 'dialog' && dialog.modal === 'true' && dialog.labelledBy === dialog.titleId,
	JSON.stringify(dialog));
record('the first field takes the focus', dialog.focus === 'offline-name', `focus=${dialog.focus}`);

// Tab from the last control wraps to the first one.
await evaluate(`document.querySelector('#modal-confirm').focus(), true`);
await pressKey('Tab', 9);
const wrapped = await evaluate('document.activeElement.id');
record('Tab stays inside the dialog', wrapped === 'offline-name', `focus=${wrapped}`);

// Escape closes it and gives the focus back to the button that opened it.
await pressKey('Escape', 27);
const closed = await evaluate(`({
	hidden: document.querySelector('#modal').hidden,
	focus: document.activeElement.id
})`);
record('Escape closes the dialog', closed.hidden === true, JSON.stringify(closed));
record('the focus goes back to the opener', closed.focus === 'btn-add-offline', `focus=${closed.focus}`);

// --- the toasts -------------------------------------------------------------------------
const toasts = await evaluate(`(() => {
	const stack = document.querySelector('#toasts');
	toast('a11y check', 'error');
	const last = stack.lastElementChild;
	return {
		role: stack.getAttribute('role'),
		live: stack.getAttribute('aria-live'),
		messageRole: last.getAttribute('role'),
		text: last.textContent
	};
})()`);
record('the toast stack is a live region', toasts.role === 'status' && toasts.live === 'polite', JSON.stringify(toasts));
record('an error is announced as an alert', toasts.messageRole === 'alert' && toasts.text === 'a11y check');

// --- the action guard -------------------------------------------------------------------
const guard = await evaluate(`(async () => {
	window.__reset = false;
	const result = await withBusy(
		() => { window.__reset = true; },
		() => Promise.reject(new Error('boom from the bridge'))
	);
	const last = document.querySelector('#toasts').lastElementChild;
	return { reset: window.__reset, result, message: last.textContent };
})()`);
record('withBusy gives the controls back after a rejection', guard.reset === true && guard.result === null, JSON.stringify(guard));
record('the rejection reaches the player', guard.message === 'boom from the bridge');

// --- command palette and workspace keyboard shortcuts ----------------------------------------
await evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', {
	key: 'k', ctrlKey: true, bubbles: true, cancelable: true
})), true`);
await waitFor(`document.querySelector('#command-palette').hidden === false`, 'the command palette');
const paletteOpens = await evaluate(`({
	open: !document.querySelector('#command-palette').hidden,
	options: document.querySelectorAll('#command-results .command-result').length,
	focused: document.activeElement.id
})`);
record('Ctrl+K opens the command palette', paletteOpens.open && paletteOpens.options >= 8 && paletteOpens.focused === 'command-search',
	JSON.stringify(paletteOpens));

await evaluate(`(() => {
	const input = document.querySelector('#command-search');
	input.value = 'performance';
	input.dispatchEvent(new Event('input', { bubbles: true }));
	return true;
})()`);
const filtered = await evaluate(`({
	count: document.querySelectorAll('#command-results .command-result').length,
	label: document.querySelector('#command-results .command-result strong')?.textContent
})`);
record('the command palette filters pages and actions', filtered.count === 1 && filtered.label === 'Performance',
	JSON.stringify(filtered));

await pressKey('Escape', 27);
record('Escape closes the command palette', await evaluate(`document.querySelector('#command-palette').hidden`) === true);
await evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', {
	key: '6', ctrlKey: true, bubbles: true, cancelable: true
})), true`);
await waitFor(`document.querySelector('.page.is-active')?.dataset.page === 'performance'`, 'the keyboard page shortcut');
record('Ctrl+6 navigates to Performance', await evaluate(`document.querySelector('.page.is-active').dataset.page`) === 'performance');

// Theme changes are written through the Rust settings command, not browser storage.
const originalTheme = await evaluate(`window.aether.settings.get().then((result) => result.settings.theme)`);
await evaluate(`document.querySelector('#theme-toggle').click(), true`);
const switchedTheme = originalTheme === 'light' ? 'dark' : 'light';
await waitFor(`document.documentElement.dataset.theme === ${JSON.stringify(switchedTheme)}`, 'the theme to change');
await waitFor(
	`window.aether.settings.get().then((result) => result.settings.theme === ${JSON.stringify(switchedTheme)})`,
	'the theme to be saved by Rust'
);
record('the selected theme is persisted by the Rust settings core', true, switchedTheme);
await evaluate(`document.querySelector('#theme-toggle').click(), true`);
await waitFor(
	`window.aether.settings.get().then((result) => result.settings.theme === ${JSON.stringify(originalTheme)})`,
	'the original theme to be restored'
);

// The dashboard must display the Java runtime resolved by the Rust core, not infer it from an
// empty custom path. Both the dashboard and Settings must use the same diagnostic object.
const runtime = await evaluate('window.aether.info().then((info) => info.javaRuntime)');
const displayedJava = await evaluate(`({
	label: document.querySelector('#dashboard-java').textContent,
	help: document.querySelector('#java-runtime-status').textContent,
	stateCompatible: window.aetherState.javaRuntime?.compatible
})`);
const expectedJavaLabel = runtime?.compatible
	? 'Java ' + runtime.majorVersion + ' · Ready'
	: runtime?.available ? 'Java version too old' : 'Java not found';
record('the Java dashboard reflects Rust runtime detection',
	displayedJava.label === expectedJavaLabel && displayedJava.stateCompatible === runtime?.compatible,
	JSON.stringify({ runtime, displayedJava }));

// A saved server appears on Home and its Join action uses the same Rust launch path as the
// full Servers page. The test address is loopback and is removed immediately afterwards.
const quickServer = await evaluate(`window.aether.servers.add('Aether QA', '127.0.0.1:25570')`);
record('a saved server can be created through the Rust command bridge', quickServer?.ok === true, quickServer?.error || '');
await evaluate(`document.querySelector('.nav-item[data-page="servers"]').click(), true`);
await waitFor(
	`document.querySelector('#quick-servers .quick-server-row .quick-server-copy strong')?.textContent === 'Aether QA'`,
	'the saved server to appear in Quick Connect'
);
const quickRow = await evaluate(`(() => {
	const row = document.querySelector('#quick-servers .quick-server-row');
	return {
		name: row?.querySelector('strong')?.textContent,
		address: row?.querySelector('.quick-server-copy small')?.textContent,
		joinDisabled: row?.querySelector('button')?.disabled
	};
})()`);
record('Quick Connect displays a saved server and enables Join for the active account',
	quickRow.name === 'Aether QA' && quickRow.address === '127.0.0.1:25570' && quickRow.joinDisabled === false,
	JSON.stringify(quickRow));
const removeQuickServer = await evaluate(`window.aether.servers.remove('127.0.0.1:25570')`);
record('a Quick Connect test server can be removed cleanly', removeQuickServer?.ok === true, removeQuickServer?.error || '');
await evaluate(`document.querySelector('#quick-servers-open').click(), true`);
await waitFor(
	`!Array.from(document.querySelectorAll('#quick-servers .quick-server-row .quick-server-copy strong')).some((node) => node.textContent === 'Aether QA')`,
	'the removed server to disappear from Quick Connect'
);

socket.close();
await sleep(200);
const failed = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failed}/${results.length} checks passed`);
process.exit(failed === 0 ? 0 : 1);
