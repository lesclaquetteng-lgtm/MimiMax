// Drives the real launcher window over the Chrome DevTools Protocol and captures one
// screenshot per screen, including the secondary states (empty lists, error toast, modal,
// download in progress, a running session, a log file) and the narrowest supported window.
//
// Usage: node screens.mjs <debugging-port> <output-directory>

import { mkdir, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9222';
const output = process.argv[3] || '.';
await mkdir(output, { recursive: true });

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
		return;
	}

	if (message.method === 'Runtime.exceptionThrown') {
		console.error('RENDERER EXCEPTION', message.params.exceptionDetails.text);
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

async function waitForText(text, timeout = 20000) {
	const start = Date.now();
	while (Date.now() - start < timeout) {
		if (await evaluate(`document.body.innerText.includes(${JSON.stringify(text)})`)) return true;
		await sleep(200);
	}
	throw new Error(`timeout while waiting for the text ${text}`);
}

/* --------------------------------------------------------------- the probe -- */

const PROBE = `(() => {
	const content = document.querySelector('.content');
	const page = document.querySelector('.page.is-active');
	const clipped = [...page.querySelectorAll('.card, .account-row, .version-row, .mod-row, .session-row, .empty, .field, .switch, .badge')]
		.filter((node) => node.scrollWidth - node.clientWidth > 1)
		.map((node) => node.className);
	const tiny = [...page.querySelectorAll('h1, h2, p, span, output, button, code')]
		.filter((node) => node.clientWidth > 0 && node.scrollWidth - node.clientWidth > 2)
		.map((node) => (node.className || node.tagName) + ':' + node.scrollWidth + '>' + node.clientWidth);
	return {
		page: page.dataset.page,
		horizontalOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
		contentOverflow: content.scrollWidth - content.clientWidth,
		clipped,
		tiny
	};
})()`;

const probes = [];

async function probe(label) {
	const result = await evaluate(PROBE);
	probes.push({ label, ...result });
	const ok = result.horizontalOverflow === 0 && result.contentOverflow === 0
		&& result.clipped.length === 0 && result.tiny.length === 0;
	console.log(`${ok ? 'OK  ' : 'BAD '} ${label} ${JSON.stringify(result)}`);
	return ok;
}

async function shot(name, keepToasts = false) {
	// A toast of a previous step must not sit on top of the screen being captured.
	if (!keepToasts) await evaluate(`document.querySelector('#toasts').innerHTML = '', true`);
	await sleep(140);
	const answer = await send('Page.captureScreenshot', { format: 'png' });
	const file = path.join(output, `${name}.png`);
	await writeFile(file, Buffer.from(answer.result.data, 'base64'));
	console.log(`captured ${file}`);
}

async function openPage(name) {
	await evaluate(`document.querySelector('.nav-item[data-page="${name}"]').click(), true`);
	await sleep(340);
}

/* -------------------------------------------------------------- the states -- */

await send('Page.enable');
await send('Runtime.enable');

await waitFor(
	`document.querySelector('#paths-output') && document.querySelector('#paths-output').textContent.length > 0`,
	'the initial state'
);
await waitFor(`document.querySelectorAll('#play-accounts .account-row').length > 0`, 'the account row');
await waitFor(
	`document.querySelectorAll('#version-table .version-row').length > 0`,
	'the version rows'
).catch(() => console.log('note: the version list stayed empty (network)'));
await waitFor(`document.querySelectorAll('#mods-table .mod-row').length > 0`, 'the mod rows');
await sleep(400);

const info = await evaluate('window.aether.info()');
console.log('paths', JSON.stringify(info.paths));

await openPage('play');
await probe('play');
await shot('01-play');

await openPage('accounts');
await probe('accounts');
await shot('02-accounts');

await openPage('versions');
await probe('versions');
await shot('03-versions');

await openPage('settings');
await probe('settings');
await shot('04-settings');

await openPage('mods');
await probe('mods');
await shot('05-mods');

// The session that the core wrote for the previous run is already on the disk.
await openPage('logs');
await waitFor(
	`document.querySelector('#sessions-table .session-row') !== null`,
	'the session log of the previous run'
);
await probe('logs');
await shot('06-logs');

// Reading it back shows exactly what the core wrote during that run.
await evaluate(`document.querySelector('#sessions-table .session-row .btn').click(), true`);
await waitFor(`document.querySelector('#session-file') !== null`, 'the session file to be read back');
await sleep(200);
await probe('logs-session-file');
await shot('07-session-file');

// The launch fills the current session of the page, the fake core prints a few lines.
await evaluate(`document.querySelector('#btn-launch').click(), true`);
await waitFor(`document.querySelector('#session-log').textContent.includes('fake java started')`, 'the session output');
await openPage('logs');
await probe('logs-session');
await shot('08-logs-session');

await evaluate('window.aether.stop(), true');
await waitFor(
	`document.querySelector('#status-text').textContent === 'Ready' && document.querySelector('#btn-stop').hidden === true`,
	'the interface to come back to the ready state',
	20000
);

// The removal leaves an empty list behind and raises a success toast.
await openPage('accounts');
await evaluate(`document.querySelector('#account-table .account-row .icon-btn').click(), true`);
await waitFor(`document.querySelectorAll('#account-table .account-row').length === 0`, 'the empty account list');
await sleep(350);
await probe('accounts-empty');
await shot('09-accounts-empty', true);

// Without any account the launch is refused, which is the error toast.
await openPage('play');
await waitFor(`document.querySelectorAll('#play-accounts .account-row').length === 0`, 'the empty play list');
await evaluate(`document.querySelector('#btn-launch').click(), true`);
await waitForText('Add an account first.');
await sleep(300);
await probe('play-empty');
await shot('10-play-empty', true);

// The account modal, filled with a valid name.
await evaluate(`document.querySelector('#play-add-offline').click(), true`);
await sleep(250);
await evaluate(`(() => {
	const input = document.querySelector('#offline-name');
	input.value = 'Notch';
	input.dispatchEvent(new Event('input', { bubbles: true }));
	input.focus();
	return true;
})()`);
await sleep(200);
await shot('11-modal');
await probe('modal');
await evaluate(`document.querySelector('#modal-cancel').click(), true`);
await sleep(200);

// The download state: the bar visible, the profile frozen and the buttons disabled.
await evaluate(`(() => {
	const progress = document.querySelector('#install-progress');
	progress.hidden = false;
	progress.querySelector('.progress-fill').style.width = '42%';
	document.querySelector('#progress-label').textContent = 'Downloading libraries (42%)';
	document.querySelector('#btn-install').disabled = true;
	document.querySelector('#select-version').disabled = true;
	document.querySelector('#select-loader').disabled = true;
	progress.closest('.card').classList.add('is-busy');
	return true;
})()`);
await sleep(250);
await probe('play-progress');
await shot('12-play-progress');
await evaluate(`(() => {
	const progress = document.querySelector('#install-progress');
	progress.hidden = true;
	document.querySelector('#btn-install').disabled = false;
	document.querySelector('#select-version').disabled = false;
	document.querySelector('#select-loader').disabled = false;
	progress.closest('.card').classList.remove('is-busy');
	return true;
})()`);

// An empty mods folder, then a log file written by an installation.
const modsFolder = info.paths.mods;
for (const name of await readdir(modsFolder)) {
	await rm(path.join(modsFolder, name), { force: true, recursive: true });
}
await openPage('mods');
await waitFor(`document.querySelectorAll('#mods-table .mod-row').length === 0`, 'the empty mods list');
await probe('mods-empty');
await shot('13-mods-empty');

// Without any session on the disk the list says so.
for (const name of await readdir(info.paths.logs)) {
	if (name.startsWith('session-')) await rm(path.join(info.paths.logs, name), { force: true });
}
await openPage('logs');
await waitFor(
	`document.querySelector('#sessions-table .empty') !== null`,
	'the empty session list'
);
await probe('sessions-empty');
await shot('14-sessions-empty');

const logFile = path.join(info.paths.logs, 'install.log');
await writeFile(
	logFile,
	[
		'download libraries/net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar — 4096 bytes (4096 fetched, 0 resumed) in 42 ms, 95.2 KiB/s',
		'download versions/1.21.11/1.21.11.json — 128 bytes (128 fetched, 0 resumed) in 3 ms, 41.7 KiB/s',
		'download assets/objects/1a/1a2b3c — 524288 bytes (393216 fetched, 131072 resumed) in 2100 ms, 187.1 KiB/s',
		'download mods/fabric-api-0.141.6+1.21.11.jar — 262144 bytes (262144 fetched, 0 resumed) in 900 ms, 284.4 KiB/s',
		'download mods/aether-client.jar — 131072 bytes (131072 fetched, 0 resumed) in 480 ms, 266.7 KiB/s'
	].join('\n') + '\n'
);
await openPage('logs');
await waitFor(`document.querySelector('#install-log') !== null`, 'the install log lines');
await probe('logs-filled');
await shot('15-logs-filled');

// The narrowest window the application allows.
await send('Emulation.setDeviceMetricsOverride', {
	width: 940,
	height: 620,
	deviceScaleFactor: 1,
	mobile: false
});
await sleep(350);

const narrow = [
	['play', '16-play-min'],
	['versions', '17-versions-min'],
	['settings', '18-settings-min'],
	['accounts', '19-accounts-min'],
	['mods', '20-mods-min'],
	['logs', '21-logs-min']
];

for (const [name, file] of narrow) {
	await openPage(name);
	await probe(`${name}-min`);
	await shot(file);
}

await send('Emulation.clearDeviceMetricsOverride');
await sleep(250);

const bad = probes.filter((entry) => entry.horizontalOverflow !== 0 || entry.contentOverflow !== 0
	|| entry.clipped.length > 0 || entry.tiny.length > 0);

console.log(`SUMMARY ${probes.length - bad.length}/${probes.length} screens without overflow`);
if (bad.length > 0) console.log(JSON.stringify(bad, null, 2));

await evaluate('window.close(), true').catch(() => {});
socket.close();
await sleep(400);
process.exit(bad.length === 0 ? 0 : 1);
