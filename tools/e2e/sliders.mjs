// Drags the real sliders of the launcher with real mouse input and reads back the CSS variable
// the track is painted with. `applySettings` alone proves nothing: this drives the input widget.
//
// Usage: node sliders.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';
import { pointAt } from './click.mjs';
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

/** The state of a slider: its value and the width of the filled part of its track. */
function state(selector) {
	return evaluate(`(() => {
		const input = document.querySelector(${JSON.stringify(selector)});
		const min = Number(input.min);
		const max = Number(input.max);
		const value = Number(input.value);
		return {
			value,
			min,
			max,
			fill: getComputedStyle(input).getPropertyValue('--fill').trim(),
			expected: Math.round(((value - min) / (max - min)) * 100) + '%',
			box: (() => { const rect = input.getBoundingClientRect(); return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }; })()
		};
	})()`);
}

/** Presses the track at `from`, drags to `to` like a player would, releases. */
async function drag(box, from, to) {
	const y = box.y + box.height / 2;
	const startX = box.x + box.width * from;
	const endX = box.x + box.width * to;

	await send('Input.dispatchMouseEvent', {
		type: 'mousePressed', x: startX, y, button: 'left', buttons: 1, clickCount: 1
	});
	await send('Input.dispatchMouseEvent', {
		type: 'mouseMoved', x: (startX + endX) / 2, y, button: 'left', buttons: 1
	});
	await send('Input.dispatchMouseEvent', {
		type: 'mouseMoved', x: endX, y, button: 'left', buttons: 1
	});
	await send('Input.dispatchMouseEvent', {
		type: 'mouseReleased', x: endX, y, button: 'left', buttons: 0, clickCount: 1
	});
	await sleep(250);
}

const results = [];

async function checkDrag(name, selector, target) {
	// The track may sit under the fold: it is brought to the middle of the page, and the point the
	// press starts from is checked, so a press on something else fails at once instead of dragging
	// nothing until the deadline.
	const start = await pointAt(evaluate, selector, 0.1);

	const before = await state(selector);
	await drag(start.box, 0.1, target);
	const after = await state(selector);

	const fill = Number.parseFloat(after.fill);
	const expected = Number.parseFloat(after.expected);
	const moved = after.value !== before.value;
	const painted = Number.isFinite(fill) && Math.abs(fill - expected) <= 2;

	results.push({ name, before: before.value, after: after.value, fill: after.fill, expected: after.expected });
	console.log(
		`${moved && painted ? 'PASS' : 'FAIL'} ${name} — value ${before.value} → ${after.value}, --fill ${after.fill} (attendu ${after.expected})`
	);

	return moved && painted;
}

await send('Runtime.enable');
await waitFor(
	`document.querySelector('#paths-output') && document.querySelector('#paths-output').textContent.length > 0`,
	'the initial state'
);
await sleep(300);

const play = await checkDrag('play: memory', '#range-memory', 0.78);

await evaluate(`document.querySelector('.nav-item[data-page="settings"]').click(), true`);
await sleep(350);
const settings = await checkDrag('settings: maximum memory', '#range-max-memory', 0.62);

const summary = { passed: results.filter((_entry, index) => [play, settings][index]).length, total: 2, results };
console.log(JSON.stringify(summary, null, 2));

await evaluate('window.close(), true').catch(() => {});
socket.close();
await sleep(400);
process.exit(play && settings ? 0 : 1);
