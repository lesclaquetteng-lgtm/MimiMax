// A real failure of the core during Install and Launch, clicked for real in a real launcher window.
//
// The launcher is started with `AETHER_FAKE_FAIL` (see fake_core.rs), so the core really prints an
// error document and stops on a non-zero code, or really dies without any document. The check then
// reads back the whole idle state: the button, the progress bar, the frozen controls and the flags.
//
// Usage:
//   node failure.mjs <port> documents   # install + launch, the core answers with an error document
//   node failure.mjs <port> crash       # launch only, the core dies without a document

import { setTimeout as sleep } from 'node:timers/promises';
import { createClick } from './click.mjs';
import { requireTestHome } from './guard.mjs';

// Never on the profile of the player who runs the check.
requireTestHome();


const port = process.argv[2] || '9222';
const mode = process.argv[3] || 'documents';
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

/** Clicks the centre of an element with the mouse, like a player does, once the point is checked. */
const { click: clickElement } = createClick({ evaluate, send }, { settle: 150 });

/** Presses Escape like a player does, the dialog must close and give the focus back. */
async function pressEscape() {
	for (const type of ['keyDown', 'keyUp']) {
		await send('Input.dispatchKeyEvent', {
			type,
			key: 'Escape',
			code: 'Escape',
			windowsVirtualKeyCode: 27,
			nativeVirtualKeyCode: 27
		});
	}
	await sleep(150);
}

// The toasts disappear after a few seconds: the check counts every error that was ever added
// instead of the ones that are still on screen.
const errorToasts = () => evaluate('window.__errorCount || 0');

const lastErrorToast = () => evaluate(`(() => {
	const list = document.querySelectorAll('#toasts .toast--error');
	const last = list[list.length - 1];
	return last ? { text: last.textContent, role: last.getAttribute('role') } : null;
})()`);

const installState = () => evaluate(`(() => ({
	buttonDisabled: document.querySelector('#btn-install').disabled,
	progressHidden: document.querySelector('#install-progress').hidden,
	fill: document.querySelector('#install-progress .progress-fill').style.width,
	versionDisabled: document.querySelector('#select-version').disabled,
	loaderDisabled: document.querySelector('#select-loader').disabled,
	cardBusy: document.querySelector('#install-progress').closest('.card').classList.contains('is-busy'),
	installing: window.aetherState.installing,
	status: document.querySelector('#status-text').textContent,
	paths: document.querySelector('#paths-output').textContent.trim()
}))()`);

const launchState = () => evaluate(`(() => ({
	buttonDisabled: document.querySelector('#btn-launch').disabled,
	stopHidden: document.querySelector('#btn-stop').hidden,
	launching: window.aetherState.launching,
	status: document.querySelector('#status-text').textContent
}))()`);

await send('Runtime.enable');
await waitFor(
	`document.querySelector('#paths-output') && document.querySelector('#paths-output').textContent.trim().length > 0`,
	'the initial state'
);
await sleep(400);

await evaluate(`(() => {
	window.__errorCount = 0;
	new MutationObserver((records) => {
		for (const record of records) {
			for (const node of record.addedNodes) {
				if (node.nodeType === 1 && node.classList.contains('toast--error')) {
					window.__errorCount += 1;
				}
			}
		}
	}).observe(document.querySelector('#toasts'), { childList: true });
	return true;
})()`);

const paths = (await installState()).paths;

// --- Install -------------------------------------------------------------------------------
if (mode === 'documents') {
	const idle = await installState();
	record('install is idle before the click',
		idle.installing === false && idle.buttonDisabled === false && idle.progressHidden === true,
		JSON.stringify({ installing: idle.installing, disabled: idle.buttonDisabled, hidden: idle.progressHidden }));

	const before = await errorToasts();
	await clickElement('#btn-install');
	await waitFor(`window.__errorCount > ${before}`, 'the error of the installation', 20000);

	const failed = await installState();
	const toast = await lastErrorToast();

	record('install: an error toast is shown', Boolean(toast) && toast.text.includes('failed on purpose'), JSON.stringify(toast));
	record('install: the toast is announced as an alert', toast?.role === 'alert');
	record('install: the button is clickable again', failed.buttonDisabled === false);
	record('install: the progress bar is hidden', failed.progressHidden === true);
	record('install: the progress bar is empty again', failed.fill === '0%' || failed.fill === '', `fill=${failed.fill}`);
	record('install: the controls are thawed', failed.versionDisabled === false && failed.loaderDisabled === false && failed.cardBusy === false);
	record('install: the flag is back to its initial state', failed.installing === false);
	record('install: the status reports the error', failed.status === 'Error', failed.status);
	record('install: the panels are still filled', failed.paths === paths && paths.length > 0);

	// The failure opens a report built from install.log, like the crash report does for a session.
	await waitFor(`document.querySelector('#modal').hidden === false`, 'the installation report', 20000);
	const report = await evaluate(`(() => ({
		title: document.querySelector('#modal-title').textContent,
		text: document.querySelector('#install-report') ? document.querySelector('#install-report').textContent : '',
		confirm: document.querySelector('#modal-confirm').textContent,
		openLogs: Boolean(document.querySelector('#report-open-logs'))
	}))()`);
	record('install: the report opens by itself', report.title === 'The installation failed', report.title);
	record('install: the report says what happened', /problem\s+.*failed on purpose/.test(report.text), report.text.split('\n')[2] || '');
	record('install: the report names the version and the log', /version\s+\d/.test(report.text) && /log\s+.*install\.log/.test(report.text));
	record('install: the report is offered for copying', report.confirm === 'Copy report', report.confirm);
	record('install: the report links to the logs', report.openLogs);

	// Escape gives the interface back, the retry must not be blocked by the dialog.
	await pressEscape();
	await waitFor(`document.querySelector('#modal').hidden === true`, 'the report closed by Escape');

	// The guard of `install()` must let a second attempt through instead of refusing it silently.
	const retryBefore = await errorToasts();
	await clickElement('#btn-install');
	await waitFor(`window.__errorCount > ${retryBefore}`, 'the error of the second installation', 20000);
	const retried = await installState();
	record('install: a second attempt runs and fails the same way',
		retried.installing === false && retried.buttonDisabled === false && retried.progressHidden === true,
		JSON.stringify({ installing: retried.installing, disabled: retried.buttonDisabled, hidden: retried.progressHidden }));

	// The second failure reports itself too; the launch checks need the interface back.
	await waitFor(`document.querySelector('#modal').hidden === false`, 'the report of the second failure', 20000);
	await pressEscape();
	await waitFor(`document.querySelector('#modal').hidden === true`, 'the second report closed by Escape');
} else {
	console.log('SKIP install, the crash mode only fails the launch');
}

// --- Launch --------------------------------------------------------------------------------
const launchIdle = await launchState();
record('launch is idle before the click',
	launchIdle.launching === false && launchIdle.buttonDisabled === false && launchIdle.stopHidden === true,
	JSON.stringify(launchIdle));

const beforeLaunch = await errorToasts();
await clickElement('#btn-launch');
	await waitFor(`window.__errorCount > ${beforeLaunch}`, 'the error of the launch', 30000);

const launched = await launchState();
const launchToast = await lastErrorToast();
const expected = mode === 'crash'
	? launched.status === 'Error' && /stopped with the code 3/.test(launchToast?.text || '')
	: /failed on purpose/.test(launchToast?.text || '');

record('launch: an error toast is shown', Boolean(launchToast) && expected, JSON.stringify(launchToast));
record('launch: the toast is announced as an alert', launchToast?.role === 'alert');
record('launch: the launch button is clickable again', launched.buttonDisabled === false);
record('launch: the stop button is hidden again', launched.stopHidden === true);
record('launch: the flag is back to its initial state', launched.launching === false);
record('launch: the status reports the error', launched.status === 'Error', launched.status);

socket.close();
await sleep(200);
const failedChecks = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failedChecks}/${results.length} checks passed`);
process.exit(failedChecks === 0 ? 0 : 1);
