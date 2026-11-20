// Proves the safety kill of the launcher.
//
// The launcher runs with `AETHER_FAKE_FAIL=install-stubborn`: the core prints progress forever and
// never reads its standard input, so it ignores `cancel` completely. The interface must therefore
// wait for its own deadline (10 s), kill the process tree and still come back to an idle state
// with a clear answer.
//
// Usage: node stubborn.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';
import { createClick } from './click.mjs';
import { requireTestHome } from './guard.mjs';

// Never on the profile of the player who runs the check.
requireTestHome();


const port = process.argv[2] || '9222';
const FALLBACK_MS = 10000;
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
		await sleep(200);
	}
	throw new Error(`timeout while waiting for ${description}`);
}

const { click: clickElement } = createClick({ evaluate, send }, { settle: 120 });

const view = () => evaluate(`(() => {
	const progress = document.querySelector('#install-progress');
	const toasts = Array.from(document.querySelectorAll('#toasts .toast'));
	return {
		progressHidden: progress.hidden,
		fill: progress.querySelector('.progress-fill').style.width,
		cancelHidden: document.querySelector('#btn-cancel-install').hidden,
		installDisabled: document.querySelector('#btn-install').disabled,
		versionDisabled: document.querySelector('#select-version').disabled,
		loaderDisabled: document.querySelector('#select-loader').disabled,
		installing: window.aetherState.installing,
		status: document.querySelector('#status-text').textContent,
		toast: toasts.length ? toasts[toasts.length - 1].textContent : ''
	};
})()`);

await send('Runtime.enable');
await waitFor(`document.querySelector('#paths-output').textContent.trim().length > 0`, 'the initial state');
await sleep(400);

await clickElement('#btn-install');
await waitFor(`!document.querySelector('#install-progress').hidden`, 'the progress bar');
await waitFor(`parseInt(document.querySelector('#progress-fill').style.width || '0', 10) > 0`, 'the progress to move', 15000);

const startedAt = Date.now();
await clickElement('#btn-cancel-install');

// The core never answers: the answer can only come from the deadline of the launcher.
await waitFor(`window.aetherState.installing === false`, 'the safety kill of the launcher', 40000);
const elapsed = (Date.now() - startedAt) / 1000;

record(
	`the launcher waited for its deadline before killing the core (${elapsed.toFixed(1)} s)`,
	elapsed * 1000 >= FALLBACK_MS - 1500 && elapsed <= 25,
	`${elapsed.toFixed(1)} s`
);

const after = await view();
record('the installation is reported as cancelled', after.toast.includes('The installation was cancelled.'), after.toast);
record('the installation button is clickable again', after.installDisabled === false);
record('the progress bar is hidden', after.progressHidden === true);
record('the progress bar is empty again', after.fill === '0%' || after.fill === '', `fill=${after.fill}`);
record('the cancel button is gone', after.cancelHidden === true);
record('the controls are thawed', after.versionDisabled === false && after.loaderDisabled === false);
record('the flag is back to its initial state', after.installing === false);
record('the status is ready again', after.status === 'Ready', after.status);

const second = await evaluate('window.aether.installStop()');
record('the core has no installation left to stop', second.ok === false, JSON.stringify(second));

socket.close();
await sleep(200);
const failed = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failed}/${results.length} checks passed`);
process.exit(failed === 0 ? 0 : 1);
