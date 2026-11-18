// Drives the real launcher window over the Chrome DevTools Protocol.
//
// The script attaches to the renderer of the launcher, calls the same bridge the interface uses
// (`window.aether`), and clicks the real buttons. Nothing is simulated: every call goes through the
// preload, the IPC handlers of the main process and the Rust core.
//
// Usage: node drive.mjs <debugging-port> <pid-file>

import { readFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9222';
const pidFile = process.argv[3];

const results = [];
const problems = [];

function check(name, condition, detail = '') {
	const passed = Boolean(condition);
	results.push({ name, passed, detail });
	console.log(`${passed ? 'PASS' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
	if (!passed) problems.push(name);
	return passed;
}

/** Waits for the debugging endpoint of the application. */
async function findPage() {
	for (let attempt = 0; attempt < 120; attempt++) {
		try {
			const answer = await fetch(`http://127.0.0.1:${port}/json/list`);
			const targets = await answer.json();
			const page = targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
			if (page) return page.webSocketDebuggerUrl;
		} catch {
			// The application is still starting.
		}
		await sleep(500);
	}

	throw new Error(`no debugging endpoint on port ${port}`);
}

const websocketUrl = await findPage();
console.log(`attached to ${websocketUrl}`);

const socket = new WebSocket(websocketUrl);
await new Promise((resolve, reject) => {
	socket.onopen = resolve;
	socket.onerror = () => reject(new Error('the websocket of the renderer refused the connection'));
});

let nextId = 1;
const pending = new Map();

socket.onmessage = (event) => {
	const message = JSON.parse(event.data);

	if (message.id && pending.has(message.id)) {
		pending.get(message.id)(message);
		pending.delete(message.id);
		return;
	}

	if (message.method === 'Runtime.exceptionThrown') {
		const details = message.params.exceptionDetails;
		console.error('RENDERER EXCEPTION', details.exception?.description || details.text);
	}

	if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
		console.error('RENDERER ERROR', message.params.args.map((arg) => arg.value ?? arg.description).join(' '));
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
	if (exception) {
		throw new Error(exception.exception?.description || exception.text);
	}

	return answer.result?.result?.value;
}

async function waitFor(expression, description, timeout = 40000) {
	const start = Date.now();

	while (Date.now() - start < timeout) {
		if (await evaluate(expression)) return true;
		await sleep(250);
	}

	throw new Error(`timeout while waiting for ${description}`);
}

function isAlive(pid) {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

await send('Runtime.enable');

/* ------------------------------------------------------------------ the boot */

await waitFor(
	`document.querySelector('#paths-output') && document.querySelector('#paths-output').textContent.length > 0`,
	'the initial state of the interface'
);

await waitFor(
	`document.querySelectorAll('#select-version option').length > 0`,
	'the version select to be filled'
);

// The list of Mojang is read over the network, it may take a moment.
const versionsRendered = await waitFor(
	`document.querySelectorAll('#version-table .version-row').length > 0`,
	'the rendered version list',
	30000
).catch(() => false);

const boot = await evaluate(`(() => ({
	status: document.querySelector('#status-text').textContent,
	paths: document.querySelector('#paths-output').textContent.trim().split('\\n'),
	accounts: document.querySelectorAll('#play-accounts .account-row').length,
	versionOptions: document.querySelectorAll('#select-version option').length,
	maxMemory: document.querySelector('#range-max-memory').value,
	g1: document.querySelector('#check-g1').checked,
	session: document.querySelector('#session-badge').textContent,
	versionRows: document.querySelectorAll('#version-table .version-row').length
}))()`);

console.log('boot', JSON.stringify(boot));

check('init(): the paths of the core are displayed', boot.paths.length === 4, boot.paths.join(' | '));
check('init(): the accounts of the store are rendered', boot.accounts > 0, `${boot.accounts} account(s)`);
check('init(): the settings are applied to the widgets', boot.maxMemory === '4096', `range-max-memory=${boot.maxMemory}`);
check('init(): the version select is usable', boot.versionOptions > 0, `${boot.versionOptions} option(s)`);
check('init(): the versions of Mojang are rendered', versionsRendered && boot.versionRows > 0, `${boot.versionRows} row(s)`);
check('init(): a session is selected', boot.session !== 'no account selected', boot.session);

/* ------------------------------------------------------------- the JSON contract */

const info = await evaluate('window.aether.info()');
check('info: the answer is a success', info && info.ok === true, info?.error || '');
check('info: the settings are usable', info?.settings?.gameVersion === '1.21.11', `gameVersion=${info?.settings?.gameVersion}`);
check('info: the paths are usable', Boolean(info?.paths?.game), info?.paths?.game);

const accounts = await evaluate('window.aether.accounts.get()');
check('accounts.get: the answer is a success', accounts && accounts.ok === true, accounts?.error || '');
check('accounts.get: the store is not empty', Array.isArray(accounts?.accounts) && accounts.accounts.length > 0, `${accounts?.accounts?.length} account(s)`);
check('accounts.get: an account is selected', Boolean(accounts?.selected), String(accounts?.selected));
check('accounts.get: the account shape reaches the interface', accounts?.accounts?.[0]?.type === 'offline' && /^[0-9a-f-]{36}$/.test(accounts?.accounts?.[0]?.uuid || ''), JSON.stringify(accounts?.accounts?.[0] || null));

const versions = await evaluate('window.aether.versions.list(5)');
check('versions: the answer is a success', versions && versions.ok === true, versions?.error || '');
check('versions: the list is usable', Array.isArray(versions?.versions) && versions.versions.length === 5, `${versions?.versions?.length} version(s)`);

/* ---------------------------------------------------------- the typed settings */

const numberSet = await evaluate('window.aether.settings.update({ maxMemory: 8192 })');
check('settings.set: a number is accepted', numberSet?.ok === true && numberSet?.settings?.maxMemory === 8192, numberSet?.error || `maxMemory=${numberSet?.settings?.maxMemory}`);

const booleanSet = await evaluate('window.aether.settings.update({ useG1GC: false })');
check('settings.set: a boolean is accepted under a differently cased key', booleanSet?.ok === true && booleanSet?.settings?.useG1gc === false, booleanSet?.error || `useG1gc=${booleanSet?.settings?.useG1gc}`);

const typo = await evaluate('window.aether.settings.update({ maxMemori: 1 })');
check('settings.set: an unknown key is refused', typo?.ok === false && /unknown setting/.test(typo?.error || ''), typo?.error || '');

const reRead = await evaluate('window.aether.settings.get()');
check('settings.get: the number is read back', reRead?.ok === true && reRead?.settings?.maxMemory === 8192, `maxMemory=${reRead?.settings?.maxMemory}`);
check('settings.get: the boolean is read back', reRead?.settings?.useG1gc === false, `useG1gc=${reRead?.settings?.useG1gc}`);

const afterWrite = await evaluate('window.aether.info()');
check('info: the new settings survive a second call', afterWrite?.settings?.maxMemory === 8192 && afterWrite?.settings?.useG1gc === false, `maxMemory=${afterWrite?.settings?.maxMemory}, useG1gc=${afterWrite?.settings?.useG1gc}`);

/* ----------------------------------------------------------- the stop of the game */

const idleStop = await evaluate('window.aether.stop()');
check('stop: nothing runs before the launch', idleStop?.ok === false, idleStop?.error || '');

await evaluate(`document.querySelector('#btn-launch').click(), true`);

await waitFor(
	`document.querySelector('#console').textContent.includes('fake java child')`,
	'the fake game to start'
);

const running = await evaluate(`(() => ({
	status: document.querySelector('#status-text').textContent,
	stopHidden: document.querySelector('#btn-stop').hidden,
	launchDisabled: document.querySelector('#btn-launch').disabled,
	console: document.querySelector('#console').textContent.trim().split('\\n')
}))()`);

console.log('running', JSON.stringify(running));

check('launch: the interface waits for a game', running.launchDisabled === true && running.stopHidden === false);
check('launch: the output of the game reaches the interface', running.console.some((line) => line.includes('fake java started')), running.console.join(' / '));

const pids = (await readFile(pidFile, 'utf8'))
	.split(/\r?\n/)
	.map((value) => Number.parseInt(value, 10))
	.filter((value) => Number.isInteger(value) && value > 0);

check('launch: the game and its child are running', pids.length === 2 && pids.every(isAlive), pids.join(', '));

const stopped = await evaluate('window.aether.stop()');
check('stop: the request is accepted', stopped?.ok === true, stopped?.error || '');

await waitFor(
	`document.querySelector('#status-text').textContent === 'Ready'
		&& document.querySelector('#btn-launch').disabled === false
		&& document.querySelector('#btn-stop').hidden === true`,
	'the interface to come back to the ready state',
	20000
);

const restored = await evaluate(`(() => ({
	status: document.querySelector('#status-text').textContent,
	launchDisabled: document.querySelector('#btn-launch').disabled,
	stopHidden: document.querySelector('#btn-stop').hidden
}))()`);

check('stop: the interface is ready again', restored.status === 'Ready' && restored.launchDisabled === false && restored.stopHidden === true, JSON.stringify(restored));

// The kill is asynchronous on Windows, both processes have a few seconds to disappear.
let dead = false;
for (let attempt = 0; attempt < 60 && !dead; attempt++) {
	dead = pids.every((pid) => !isAlive(pid));
	if (!dead) await sleep(250);
}

check('stop: the game and its child are gone', dead, pids.map((pid) => `${pid}:${isAlive(pid) ? 'alive' : 'dead'}`).join(', '));

const stopAfter = await evaluate('window.aether.stop()');
check('stop: a second stop reports that nothing runs', stopAfter?.ok === false, stopAfter?.error || '');

/* -------------------------------------------------------------- the mods */

const mods = await evaluate('window.aether.mods.list()');
check('mods: the answer is a success', mods?.ok === true, mods?.error || '');
check('mods: the folder is known', typeof mods?.folder === 'string' && mods.folder.length > 0, mods?.folder);
check('mods: the jars of the folder are listed', Array.isArray(mods?.mods) && mods.mods.length > 0, `${mods?.mods?.length} mod(s)`);

const first = mods?.mods?.[0];
const toggled = first
	? await evaluate(`window.aether.mods.toggle(${JSON.stringify(first.name)}, ${!first.enabled})`)
	: null;
check(
	'mods: a mod is disabled and enabled again',
	Boolean(first) && toggled?.ok === true && toggled?.mod?.enabled === !first.enabled,
	first ? `${first.name} → ${toggled?.mod?.name}` : 'no mod'
);

const back = toggled?.mod
	? await evaluate(`window.aether.mods.toggle(${JSON.stringify(toggled.mod.name)}, ${Boolean(first.enabled)})`)
	: null;
check(
	'mods: the file comes back to its first name',
	Boolean(back) && back?.ok === true && back?.mod?.name === first?.name,
	back?.mod?.name || ''
);

const refused = await evaluate(`window.aether.mods.toggle('../aether/settings.json', false)`);
check(
	'mods: a name outside the folder is refused',
	refused?.ok === false && /invalid mod name/.test(refused?.error || ''),
	refused?.error || ''
);

/* -------------------------------------------------------------- the logs */

const logTail = await evaluate('window.aether.logs.tail(20)');
check('logs: the answer is a success', logTail?.ok === true, logTail?.error || '');
check(
	'logs: the installation log is readable',
	Boolean(logTail?.install) && Array.isArray(logTail.install.lines) && typeof logTail.install.path === 'string',
	String(logTail?.install?.path || '')
);

const sessionNames = logTail?.sessions || [];
check('logs: the session logs of the disk are listed', sessionNames.length > 0, `${sessionNames.length} session(s)`);

const sessionName = sessionNames[0]?.name;
const readBack = sessionName
	? await evaluate(`window.aether.logs.read(${JSON.stringify(sessionName)}, 50)`)
	: null;
const sessionLines = readBack?.session?.lines || [];
check(
	'logs: a session file is read back with the header the core writes',
	readBack?.ok === true && sessionLines.some((line) => line.startsWith('# Aether session')),
	sessionName || 'no session'
);
check(
	'logs: the session ends with the exit code of the game',
	readBack?.ok === true && sessionLines.some((line) => line.startsWith('# exit code')),
	sessionLines[sessionLines.length - 1] || ''
);

const sessionRefused = await evaluate(`window.aether.logs.read('../aether/settings.json', 10)`);
check(
	'logs: a session name outside the folder is refused',
	sessionRefused?.ok === false && /invalid session name/.test(sessionRefused?.error || ''),
	sessionRefused?.error || ''
);

/* --------------------------------------------------------------------- summary */

const passed = results.filter((result) => result.passed).length;
console.log(`SUMMARY ${passed}/${results.length} checks passed`);
console.log(JSON.stringify({ passed, total: results.length, problems }, null, 2));

// Closing the window quits the application, the process tree does not stay behind.
await evaluate('window.close(), true').catch(() => {});
socket.close();
await sleep(500);

process.exit(problems.length === 0 ? 0 : 1);
