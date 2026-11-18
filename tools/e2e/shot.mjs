// Captures the interface of the running launcher as a PNG, for a visual check.
//
// Usage: node shot.mjs <debugging-port> <output.png>

import { writeFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9228';
const output = process.argv[3];

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

// The interface is ready once the accounts of the store are rendered.
for (let attempt = 0; attempt < 120; attempt++) {
	const probe = await send('Runtime.evaluate', {
		expression: `document.querySelectorAll('#play-accounts .account-row').length > 0`,
		returnByValue: true
	});
	if (probe.result?.result?.value) break;
	await sleep(250);
}

const layout = await send('Runtime.evaluate', {
	expression: `(() => {
		const content = document.querySelector('.content');
		const progress = document.querySelector('#install-progress');
		return {
			horizontalOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
			contentOverflow: content.scrollWidth - content.clientWidth,
			progressHidden: progress.hidden,
			progressDisplay: getComputedStyle(progress).display
		};
	})()`,
	returnByValue: true
});
console.log('layout', JSON.stringify(layout.result.result.value));

await send('Page.enable');
const shot = await send('Page.captureScreenshot', { format: 'png' });
await writeFile(output, Buffer.from(shot.result.data, 'base64'));
console.log(`screenshot written to ${output}`);

socket.close();
