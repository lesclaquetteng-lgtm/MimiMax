// Drives the Aether launcher through its pages, checks the layout and captures screenshots.
//
// Usage: node ui-check.mjs <debugging-port> <output-directory>
//
// The launcher runs with an isolated AETHER_HOME, so the checks never touch the real installation.

import { mkdir, writeFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9231';
const output = process.argv[3] || 'shots';

await mkdir(output, { recursive: true });

async function findPage() {
	for (let attempt = 0; attempt < 160; attempt++) {
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
		returnByValue: true,
		awaitPromise: true
	});
	if (answer.result?.exceptionDetails) {
		throw new Error(answer.result.exceptionDetails.exception?.description || answer.result.exceptionDetails.text || 'the page threw');
	}
	return answer.result?.result?.value;
}

async function waitFor(expression, timeout = 60000, label = expression) {
	const started = Date.now();
	while (Date.now() - started < timeout) {
		if (await evaluate(expression)) return true;
		await sleep(250);
	}
	throw new Error(`timed out waiting for ${label}`);
}

async function size(width, height) {
	await send('Emulation.setDeviceMetricsOverride', {
		width,
		height,
		deviceScaleFactor: 1,
		mobile: false
	});
	await sleep(200);
}

async function screenshot(name) {
	const shot = await send('Page.captureScreenshot', { format: 'png' });
	await writeFile(`${output}/${name}.png`, Buffer.from(shot.result.data, 'base64'));
}

/** The elements that overflow their own box, the usual cause of a cut label. */
const OVERFLOW_EXPRESSION = `(() => {
	const content = document.querySelector('.content');
	const page = document.querySelector('.page.is-active');
	const offenders = [];
	const seen = new Set();
	for (const element of document.querySelectorAll('.page.is-active *')) {
		if (element.closest('.console')) continue;
		if (element.matches('input, textarea, .visually-hidden')) continue;
		// A line that ends with an ellipsis is cut on purpose, it is not a layout defect.
		if (getComputedStyle(element).textOverflow === 'ellipsis') continue;
		if (element.scrollWidth > element.clientWidth + 1 && element.clientWidth > 0) {
			const key = element.tagName + '.' + element.className;
			if (seen.has(key)) continue;
			seen.add(key);
			offenders.push({ key, scrollWidth: element.scrollWidth, clientWidth: element.clientWidth });
		}
	}
	return {
		page: page?.dataset.page || null,
		viewport: { width: innerWidth, height: innerHeight },
		documentOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
		contentOverflow: content.scrollWidth - content.clientWidth,
		contentVertical: content.scrollHeight - content.clientHeight,
		offenders: offenders.slice(0, 8)
	};
})()`;

async function openPage(name) {
	const button = await evaluate(`Boolean(document.querySelector('.nav-item[data-page="${name}"]'))`);
	if (!button) throw new Error(`navigation destination is missing: ${name}`);
	console.log(`[PAGE] ${name}`);
	await evaluate(`document.querySelector('.nav-item[data-page="${name}"]').click()`);
	await sleep(350);
}

await send('Page.enable');
await waitFor('window.aetherState && window.aetherState.settings && window.aetherState.settings.gameVersion', 120000, 'the initial state');

const report = { layout: [], pages: {}, interactions: {} };

const pages = ['play', 'accounts', 'worlds', 'mods', 'servers', 'performance', 'logs', 'settings'];

for (const [width, height, suffix] of [[1180, 760, 'wide'], [940, 620, 'narrow']]) {
	await size(width, height);

	for (const name of pages) {
		await openPage(name);
		const layout = await evaluate(OVERFLOW_EXPRESSION);
		if (layout.documentOverflow > 0 || layout.contentOverflow > 0) {
			throw new Error(`horizontal page overflow at ${name} (${width}x${height})`);
		}
		report.layout.push({ ...layout, size: `${width}x${height}` });
		await screenshot(`${name}-${suffix}`);
	}
}

// The contents of the new pages, read from the real core.
await size(1180, 760);

await openPage('servers');
report.pages.servers = await evaluate(`(() => ({
	buttons: [...document.querySelectorAll('.page.is-active button')].map((b) => b.textContent.trim()).filter(Boolean),
	serverRows: document.querySelectorAll('#server-list .mod-row').length,
	hostRows: document.querySelectorAll('#host-list .mod-row').length,
	hostText: [...document.querySelectorAll('#host-list .mod-row')].map((row) => row.innerText),
	count: document.querySelector('#server-count').textContent,
	hostCount: document.querySelector('#host-count').textContent
}))()`);

await openPage('performance');
report.pages.performance = await evaluate(`(() => ({
	rows: document.querySelectorAll('#perf-mods .mod-row').length,
	installed: [...document.querySelectorAll('#perf-mods .mod-row')].filter((row) => !row.classList.contains('is-disabled')).length,
	count: document.querySelector('#perf-count').textContent,
	jvm: document.querySelector('#jvm-current').textContent,
	options: document.querySelector('#options-current').textContent,
	path: document.querySelector('#perf-path').textContent
}))()`);

await openPage('mods');
report.pages.mods = await evaluate(`(() => ({
	count: document.querySelector('#mods-count').textContent,
	rows: document.querySelectorAll('#mods-table .mod-row').length,
	names: [...document.querySelectorAll('#mods-table .mod-name')].map((node) => node.textContent)
}))()`);

// Add a real server through the form, then probe it over the network.
await openPage('servers');

// The check is run more than once on the same home: the previous entry is dropped first.
await evaluate(`(() => {
	const rows = [...document.querySelectorAll('#server-list .mod-row')];
	const row = rows.find((entry) => entry.innerText.includes('mc.hypixel.net'));
	if (!row) return;
	const remove = [...row.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Retirer');
	remove.click();
})()`);
await waitFor(`![...document.querySelectorAll('#server-list .mod-row')].some((row) => row.innerText.includes('mc.hypixel.net'))`, 15000, 'the old row to go');

const before = await evaluate(`document.querySelectorAll('#server-list .mod-row').length`);

await evaluate(`(() => {
	document.querySelector('#server-name').value = 'Hypixel';
	document.querySelector('#server-address').value = 'mc.hypixel.net';
	document.querySelector('#btn-server-add').click();
})()`);

await waitFor(`document.querySelectorAll('#server-list .mod-row').length > ${before}`, 30000, 'the new server row');

await evaluate(`(() => {
	const rows = [...document.querySelectorAll('#server-list .mod-row')];
	const row = rows.find((entry) => entry.innerText.includes('mc.hypixel.net'));
	const test = [...row.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Tester');
	test.click();
})()`);

await waitFor(`(() => {
	const text = document.querySelector('#server-verdict').textContent;
	return text.length > 0 && !text.startsWith('Test de ');
})()`, 60000, 'the verdict of the probe');

report.interactions.probe = await evaluate(`document.querySelector('#server-verdict').textContent`);
report.interactions.serverRow = await evaluate(`(() => {
	const row = [...document.querySelectorAll('#server-list .mod-row')].find((entry) => entry.innerText.includes('mc.hypixel.net'));
	return { text: row.innerText, buttons: [...row.querySelectorAll('button')].map((button) => button.textContent.trim()) };
})()`);
// The toasts cover the bottom right corner, they are given the time to leave before the shot.
await sleep(5000);
await screenshot('servers-probed-wide');

// The narrow pass with the real content, the longest row is the one that breaks first.
await size(940, 620);
await openPage('servers');
report.layout.push({ ...(await evaluate(OVERFLOW_EXPRESSION)), size: '940x620' });
await screenshot('servers-probed-narrow');

await openPage('performance');
report.layout.push({ ...(await evaluate(OVERFLOW_EXPRESSION)), size: '940x620' });
await screenshot('performance-narrow');

await size(1180, 760);
await openPage('performance');

await send('Emulation.clearDeviceMetricsOverride');
socket.close();

await writeFile(`${output}/ui-report.json`, JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
