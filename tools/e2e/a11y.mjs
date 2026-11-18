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

socket.close();
await sleep(200);
const failed = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failed}/${results.length} checks passed`);
process.exit(failed === 0 ? 0 : 1);
