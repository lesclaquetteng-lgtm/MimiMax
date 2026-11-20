// Proof of the timeout of the core: the launcher is started with a core that hangs (hang.exe), the
// interface must report it and stop waiting instead of leaving an empty panel forever.
//
// Usage: node timeout.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';
import { requireTestHome } from './guard.mjs';

// Never on the profile of the player who runs the check.
requireTestHome();


const port = process.argv[2] || '9222';

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
	const answer = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
	return answer.result?.result?.value;
}

await send('Runtime.enable');

const started = Date.now();
let state = null;

// The interface waits 20 s before it gives up, the poll leaves it a margin.
for (let attempt = 0; attempt < 300; attempt++) {
	state = await evaluate(`(() => {
		const paths = document.querySelector('#paths-output').textContent.trim();
		const toasts = Array.from(document.querySelectorAll('#toasts .toast'));
		const last = toasts[toasts.length - 1];
		return {
			paths,
			toast: last ? last.textContent : '',
			role: last ? last.getAttribute('role') : '',
			status: document.querySelector('#status-text').textContent
		};
	})()`);

	if (state.paths.includes('did not answer')) break;
	await sleep(500);
}

const seconds = Math.round((Date.now() - started) / 1000);
const reported = Boolean(state.paths.includes('did not answer'));
const announced = Boolean(state.toast.includes('did not answer')) && state.role === 'alert';

console.log(`${reported ? 'PASS' : 'FAIL'} the timeout is reported in the paths panel — ${JSON.stringify(state.paths)}`);
console.log(`${announced ? 'PASS' : 'FAIL'} the timeout is announced and visible — ${JSON.stringify(state.toast)} (role=${state.role})`);
console.log(`INFO waited ${seconds} s, status="${state.status}"`);
console.log(`SUMMARY ${(reported ? 1 : 0) + (announced ? 1 : 0)}/2 checks passed`);

socket.close();
await sleep(200);
process.exit(reported && announced ? 0 : 1);
