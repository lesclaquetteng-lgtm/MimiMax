// Checks the report the launcher shows when a session ends on a non-zero code.
//
// The launcher runs with `AETHER_FAKE_FAIL=launch-exit`: the fake core answers a launch with
// `{ "ok": true, "exitCode": 3 }`, exactly like the real core does when the game dies.
//
// Usage: node crash-report.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9222';
// `game` is the game that dies, `core` is the core that dies without answering.
const mode = process.argv[3] || 'game';
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
	await sleep(150);
}

await send('Runtime.enable');
await waitFor(`document.querySelector('#paths-output').textContent.trim().length > 0`, 'the initial state');
await sleep(400);

record('no report is shown before a launch', await evaluate(`document.querySelector('#modal').hidden === true`));

await clickElement('#btn-launch');
await waitFor(`document.querySelector('#modal').hidden === false`, 'the report of the crash', 30000);

const report = await evaluate(`(() => ({
	title: document.querySelector('#modal-title').textContent,
	text: document.querySelector('#crash-report') ? document.querySelector('#crash-report').textContent : '',
	confirm: document.querySelector('#modal-confirm').textContent,
	launching: window.aetherState.launching,
	launchDisabled: document.querySelector('#btn-launch').disabled
}))()`);

const expected = mode === 'core'
	? {
		title: 'The launcher core stopped',
		problem: /problem\s+The launcher core stopped with the code 3\./,
		code: false
	}
	: {
		title: 'The game stopped',
		problem: /problem\s+the game stopped with the code 3/,
		code: true
	};

record('the report opens by itself', report.title === expected.title, report.title);
record('the report says what happened', expected.problem.test(report.text), report.text.split('\n')[2] || '');
record(
	`the exit code line ${expected.code ? 'is' : 'is not'} in the report`,
	/exit code\s+3/.test(report.text) === expected.code,
	report.text.split('\n').slice(2, 4).join(' | ')
);
record('the version and the loader are in the report',
	/version\s+1\.21\.11/.test(report.text) && /loader\s+\d/.test(report.text),
	`${report.text.split('\n')[3]} | ${report.text.split('\n')[4]}`);
record('the session file of the crash is named', /session\s+session-\d{8}-\d{6}\.log/.test(report.text), report.text.split('\n')[6] || '');
record('the last lines of the session are included', /last lines/.test(report.text) && report.text.split('\n').length > 9);
record('the report is offered for copying', report.confirm === 'Copy report', report.confirm);
record('the interface is idle behind the report', report.launching === false && report.launchDisabled === false,
	JSON.stringify({ launching: report.launching, disabled: report.launchDisabled }));

await clickElement('#modal-confirm');
// The toast of the crash is still on screen: the confirmation is looked for among all of them.
await waitFor(
	`Array.from(document.querySelectorAll('#toasts .toast')).some((toast) => toast.textContent.includes('copied'))`,
	'the confirmation of the copy',
	15000
);

const copied = await evaluate(`(() => {
	const toasts = Array.from(document.querySelectorAll('#toasts .toast')).map((toast) => toast.textContent);
	return { modalHidden: document.querySelector('#modal').hidden, last: toasts[toasts.length - 1] };
})()`);
record('the report is copied and the dialog closes', copied.modalHidden === true && copied.last === 'The report was copied.', JSON.stringify(copied));

// --- Open the logs -------------------------------------------------------------------------
// The report also leads to the session it was built from: one click instead of a search.
await clickElement('#btn-launch');
await waitFor(`document.querySelector('#modal').hidden === false`, 'the report of the second crash', 30000);
await clickElement('#report-open-logs');
await waitFor(`document.querySelector('#modal').hidden === true`, 'the report closed by the link');

const logsPage = await evaluate(`(() => ({
	page: document.querySelector('.page.is-active').dataset.page,
	nav: document.querySelector('.nav-item.is-active').dataset.page
}))()`);
record('the link opens the logs page', logsPage.page === 'logs' && logsPage.nav === 'logs', JSON.stringify(logsPage));

const named = (report.text.match(/session\s+(session-\d{8}-\d{6}\.log)/) || [])[1];
if (named) {
	await waitFor(
		`document.querySelector('#session-view .log-path') && document.querySelector('#session-view .log-path').textContent.includes(${JSON.stringify(named)})`,
		'the session of the report',
		15000
	);
	record('the link opens the session the report was built from', true, named);
}

socket.close();
await sleep(200);
const failed = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failed}/${results.length} checks passed`);
process.exit(failed === 0 ? 0 : 1);
