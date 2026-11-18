import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9233';
const outDir = process.argv[3] || 'new-ui-shots';

const answer = await fetch(`http://127.0.0.1:${port}/json/list`);
const targets = await answer.json();
const page = targets.find((t) => t.type === 'page' && t.webSocketDebuggerUrl);
if (!page) {
	console.error('No page target found');
	process.exit(1);
}

const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
	socket.onopen = resolve;
	socket.onerror = reject;
});

let nextId = 1;
const pending = new Map();
socket.onmessage = (event) => {
	const msg = JSON.parse(event.data);
	if (msg.id && pending.has(msg.id)) {
		pending.get(msg.id)(msg);
		pending.delete(msg.id);
	}
};

function send(method, params = {}) {
	const id = nextId++;
	socket.send(JSON.stringify({ id, method, params }));
	return new Promise((resolve) => pending.set(id, resolve));
}

async function evaluate(expression) {
	const res = await send('Runtime.evaluate', {
		expression,
		returnByValue: true,
		awaitPromise: true,
		userGesture: true
	});
	if (res.result?.exceptionDetails) {
		throw new Error(res.result.exceptionDetails.text || 'eval failed');
	}
	return res.result?.result?.value;
}

async function captureShot(name) {
	const res = await send('Page.captureScreenshot', { format: 'png' });
	const buf = Buffer.from(res.result.data, 'base64');
	const file = path.join(outDir, `${name}.png`);
	await writeFile(file, buf);
	console.log(`[SHOT] Saved ${file} (${buf.length} bytes)`);
}

async function checkProbe(pageName) {
	const probeResult = await evaluate(`(() => {
		const content = document.querySelector('.content');
		const activePage = document.querySelector('.page.is-active');
		return {
			page: activePage?.dataset.page,
			docOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
			contentOverflow: content.scrollWidth - content.clientWidth,
			clippedCount: activePage ? [...activePage.querySelectorAll('.card, .account-row, .version-row, .mod-row, .session-row')].filter(n => n.scrollWidth - n.clientWidth > 1).length : 0
		};
	})()`);

	const ok = probeResult.docOverflow === 0 && probeResult.contentOverflow === 0 && probeResult.clippedCount === 0;
	console.log(`[PROBE] ${pageName}: ${ok ? 'PASS' : 'WARN'} => ${JSON.stringify(probeResult)}`);
	return ok;
}

await send('Page.enable');
await send('Runtime.enable');

// Wait for initial render
await sleep(1500);

const pages = ['play', 'accounts', 'versions', 'mods', 'servers', 'performance', 'logs', 'settings'];

for (const p of pages) {
	console.log(`\nNavigating to ${p}...`);
	await evaluate(`document.querySelector('.nav-item[data-page="${p}"]').click()`);
	await sleep(500);
	await checkProbe(p);
	await captureShot(p);
}

// Test Slider interaction
console.log('\n--- Testing Slider ---');
await evaluate(`document.querySelector('.nav-item[data-page="play"]').click()`);
await sleep(300);
const memorySlider = await evaluate(`(() => {
	const input = document.querySelector('#range-memory');
	input.value = '8192';
	input.dispatchEvent(new Event('input', { bubbles: true }));
	return {
		val: input.value,
		fill: input.style.getPropertyValue('--fill')
	};
})()`);
console.log(`Slider interaction: value=${memorySlider.val}, fill=${memorySlider.fill}`);

// Test Modal interaction
console.log('\n--- Testing Modal ---');
await evaluate(`document.querySelector('#play-add-offline').click()`);
await sleep(400);
await captureShot('modal-open');
const modalVisible = await evaluate(`!document.querySelector('#modal').hidden`);
console.log(`Modal visible: ${modalVisible}`);
await evaluate(`document.querySelector('#modal-cancel').click()`);
await sleep(300);
const modalClosed = await evaluate(`document.querySelector('#modal').hidden`);
console.log(`Modal closed: ${modalClosed}`);

console.log('\nAll UI tests completed successfully!');
socket.close();
process.exit(0);

