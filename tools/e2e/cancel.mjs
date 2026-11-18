// Cancels a real installation from the interface.
//
// The launcher runs with `AETHER_FAKE_FAIL=install-slow`: the core prints progress every 200 ms and
// only stops when it reads `cancel` on its standard input, which is exactly the production channel.
// The check clicks Install, waits for the bar to move, clicks Cancel and reads the whole idle state.
//
// Usage: node cancel.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9222';
const results = [];

function record(name, ok, detail = '') {
	results.push({ name, ok });
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
		await sleep(150);
	}
	throw new Error(`timeout while waiting for ${description}`);
}

async function clickElement(selector) {
	const box = await evaluate(`(() => {
		const element = document.querySelector(${JSON.stringify(selector)});
		const rect = element.getBoundingClientRect();
		return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2, disabled: element.disabled, hidden: element.hidden };
	})()`);

	if (box.disabled || box.hidden) throw new Error(`${selector} cannot be clicked (disabled or hidden)`);

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
	await sleep(120);
}

const view = () => evaluate(`(() => {
	const progress = document.querySelector('#install-progress');
	const cancel = document.querySelector('#btn-cancel-install');
	const toasts = Array.from(document.querySelectorAll('#toasts .toast'));
	return {
		progressHidden: progress.hidden,
		fill: progress.querySelector('.progress-fill').style.width,
		label: document.querySelector('#progress-label').textContent,
		cancelHidden: cancel.hidden,
		cancelDisabled: cancel.disabled,
		installDisabled: document.querySelector('#btn-install').disabled,
		versionDisabled: document.querySelector('#select-version').disabled,
		loaderDisabled: document.querySelector('#select-loader').disabled,
		cardBusy: progress.closest('.card').classList.contains('is-busy'),
		installing: window.aetherState.installing,
		status: document.querySelector('#status-text').textContent,
		toast: toasts.length ? toasts[toasts.length - 1].textContent : ''
	};
})()`);

await send('Runtime.enable');
await waitFor(
	`document.querySelector('#paths-output').textContent.trim().length > 0`,
	'the initial state'
);
await sleep(400);

record('the installation starts idle', (await view()).installing === false);

// --- run an installation, then cancel it -----------------------------------------------------
await clickElement('#btn-install');
await waitFor(`!document.querySelector('#install-progress').hidden`, 'the progress bar');
await waitFor(
	`parseInt(document.querySelector('#progress-fill').style.width || '0', 10) > 0`,
	'the progress to move',
	15000
);

const running = await view();
record('the bar moves while the installation runs', parseInt(running.fill, 10) > 0, `fill=${running.fill}`);
record('the cancel button is offered during the installation',
	running.cancelHidden === false && running.installing === true,
	JSON.stringify({ hidden: running.cancelHidden, installing: running.installing }));

await clickElement('#btn-cancel-install');
await waitFor(
	`Array.from(document.querySelectorAll('#toasts .toast')).some((toast) => toast.textContent.includes('cancelled'))`,
	'the cancellation answer',
	20000
);

const stopped = await view();
record('the cancellation is reported', stopped.toast.includes('The installation was cancelled.'), stopped.toast);
record('the installation button is clickable again', stopped.installDisabled === false);
record('the progress bar is hidden', stopped.progressHidden === true);
record('the progress bar is empty again', stopped.fill === '0%' || stopped.fill === '', `fill=${stopped.fill}`);
record('the cancel button is gone', stopped.cancelHidden === true);
record('the controls are thawed', stopped.versionDisabled === false && stopped.loaderDisabled === false && stopped.cardBusy === false);
record('the flag is back to its initial state', stopped.installing === false);
record('the status is ready again', stopped.status === 'Ready', stopped.status);

// Nothing is left running: a second stop has nothing to stop.
const second = await evaluate('window.aether.installStop()');
record('the core has no installation left to stop', second.ok === false, JSON.stringify(second));

socket.close();
await sleep(200);
const failed = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failed}/${results.length} checks passed`);
process.exit(failed === 0 ? 0 : 1);
