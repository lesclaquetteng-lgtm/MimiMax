// Proves the guard of the e2e clicks, in both directions.
//
// The click helpers check the point they are about to press with `document.elementFromPoint`
// (`tools/e2e/click.mjs`). This check drives that guard against the real page: a covered button must
// stop the click before any event is sent, an obstacle that disappears must give the button back,
// and a missing, hidden, disabled or off-screen control must be refused by name — all of it in
// milliseconds, where a click that landed on nothing used to wait for the 30 s timeout.
//
// Every refusal is read through `expectRefusal`, which throws when the click went through: a guard
// that stopped refusing cannot leave this scenario green.
//
// Usage: node hit.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';
import { createClick, pointAt } from './click.mjs';
import { requireTestHome } from './guard.mjs';

// Never on the profile of the player who runs the check.
requireTestHome();


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

const { aim, click } = createClick({ evaluate, send }, { settle: 50 });

/**
 * Runs a click that must be refused, and measures how fast the refusal came. An accepted click is a
 * hard failure of this check, never a silent pass.
 */
async function expectRefusal(run) {
	const started = Date.now();
	let message = '';
	try {
		await run();
	} catch (error) {
		message = error.message;
	}
	const elapsed = Date.now() - started;
	if (!message) throw new Error(`the click went through, the guard refused nothing (${elapsed} ms)`);
	return { error: message, elapsed };
}

/** Adds a probe to the page, removed with `clear()`: the launcher itself is never modified. */
async function probe(html) {
	await evaluate(`(() => {
		const host = document.createElement('div');
		host.id = 'probe-host';
		host.innerHTML = ${JSON.stringify(html)};
		document.body.append(host);
		return true;
	})()`);
}

async function clear() {
	await evaluate(`(() => {
		const host = document.querySelector('#probe-host');
		if (host) host.remove();
		return true;
	})()`);
}

await send('Runtime.enable');
await waitFor(
	`document.querySelector('#paths-output') && document.querySelector('#paths-output').textContent.trim().length > 0`,
	'the initial state'
);
await sleep(400);
await clear();

// --- the button of the launcher, reachable ---------------------------------------------------
const first = await aim('#btn-install');
record(
	'the install button can be hit at its centre',
	first.reachable === true,
	`x=${Math.round(first.x)} y=${Math.round(first.y)} hit=${first.hit}`
);

// --- covered: refused before any event -------------------------------------------------------
await probe('<div id="probe-overlay" style="position:fixed;inset:0;z-index:99999;background:transparent"></div>');
const covered = await expectRefusal(() => click('#btn-install'));
record(
	'a covered button stops the click',
	/is covered by #probe-overlay/.test(covered.error),
	covered.error
);
record('the refusal names the point it would have hit', /at -?\d+,-?\d+$/.test(covered.error), covered.error);

const untouched = await evaluate(`(() => ({
	installing: window.aetherState.installing,
	progressHidden: document.querySelector('#install-progress').hidden,
	buttonDisabled: document.querySelector('#btn-install').disabled
}))()`);
record(
	'no mouse event was sent to the covered button',
	untouched.installing === false && untouched.progressHidden === true && untouched.buttonDisabled === false,
	JSON.stringify(untouched)
);

// --- the obstacle removed: the same button is reachable again ---------------------------------
await clear();
const again = await aim('#btn-install');
record('the button is reachable again once the obstacle is gone', again.reachable === true, `hit=${again.hit}`);

// --- a hit on a child of the target counts as the target -------------------------------------
await probe('<button id="probe-parent" style="position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);z-index:2147483647;width:220px;height:40px"><span id="probe-child" style="display:block;width:100%;height:100%">inside</span></button>');
const child = await aim('#probe-parent');
record('a hit on a child of the target counts as the target', child.hit === '#probe-child', `hit=${child.hit}`);
await clear();

// --- outside the window ----------------------------------------------------------------------
await probe('<button id="probe-offscreen" style="position:fixed;left:20px;top:-400px;width:140px;height:40px">above</button>');
const offscreen = await expectRefusal(() => aim('#probe-offscreen'));
record('an element outside the window is refused', /is outside the window/.test(offscreen.error), offscreen.error);
await clear();

// --- hidden, disabled, missing ---------------------------------------------------------------
const hidden = await expectRefusal(() => aim('#btn-stop'));
record('a hidden control is refused by name', /is hidden/.test(hidden.error), hidden.error);

await probe('<button id="probe-disabled" disabled style="position:fixed;left:12px;top:140px;width:220px;height:40px">no</button>');
const disabled = await expectRefusal(() => aim('#probe-disabled'));
record('a disabled control is refused by name', /is disabled/.test(disabled.error), disabled.error);
await clear();

const missing = await expectRefusal(() => aim('#probe-absent'));
record('a missing control is refused by name', /is not in the page/.test(missing.error), missing.error);

// --- every refusal is immediate --------------------------------------------------------------
const refusals = [covered, offscreen, hidden, disabled, missing];
const slowest = Math.max(...refusals.map((entry) => entry.elapsed));
record(
	'every refusal comes back under a second, not after the timeout',
	refusals.every((entry) => typeof entry.error === 'string' && entry.error.length > 0) && slowest < 1000,
	`slowest ${slowest} ms (timeout of a check: 30 000 ms)`
);

// --- the page is left as it was found --------------------------------------------------------
const left = await evaluate(`(() => ({
	probes: document.querySelectorAll('#probe-host').length,
	reachable: !!document.elementFromPoint(innerWidth / 2, innerHeight / 2)
}))()`);
record('the probes are gone and the page still answers', left.probes === 0 && left.reachable === true, JSON.stringify(left));

socket.close();
await sleep(200);
const failedChecks = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failedChecks}/${results.length} checks passed`);
process.exit(failedChecks === 0 ? 0 : 1);
