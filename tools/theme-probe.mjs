// Reads the computed styles of the real launcher window over the DevTools protocol of WebView2.
//
// A screenshot shows that something changed; this shows *what* the engine resolved, which is the
// difference between "the theme file was edited" and "the theme is applied". Start the shell with
// `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9244"`, then:
//
//   node tools/theme-probe.mjs 9244

import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9244';

async function findPage() {
	for (let attempt = 0; attempt < 80; attempt++) {
		try {
			const answer = await fetch(`http://127.0.0.1:${port}/json/list`);
			const targets = await answer.json();
			const page = targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
			if (page) return page.webSocketDebuggerUrl;
		} catch {
			// The window is still starting.
		}
		await sleep(500);
	}

	throw new Error(`no debugging endpoint on port ${port}`);
}

const socket = new WebSocket(await findPage());
await new Promise((resolve, reject) => {
	socket.onopen = resolve;
	socket.onerror = () => reject(new Error('the debugging endpoint refused the connection'));
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

function evaluate(expression) {
	const id = nextId++;
	socket.send(
		JSON.stringify({
			id,
			method: 'Runtime.evaluate',
			params: { expression, returnByValue: true, awaitPromise: true },
		}),
	);

	return new Promise((resolve) => pending.set(id, resolve));
}

const probe = `(() => {
	const root = getComputedStyle(document.documentElement);
	const body = getComputedStyle(document.body);
	const styleOf = (selector) => {
		const element = document.querySelector(selector);
		return element ? getComputedStyle(element) : null;
	};

	const card = styleOf('.card');
	const button = styleOf('.btn--primary');
	const active = document.querySelector('.nav-item.is-active, .nav-item[aria-current="true"]');
	const missing = [];

	for (const sheet of document.styleSheets) {
		let rules;
		try { rules = sheet.cssRules; } catch { continue; }
		for (const rule of rules) {
			if (!rule.style) continue;
			for (const property of rule.style) {
				const value = rule.style.getPropertyValue(property);
				if (value.includes('var(--') && value.includes('undefined')) missing.push(rule.selectorText);
			}
		}
	}

	return JSON.stringify({
		accent: root.getPropertyValue('--accent').trim(),
		accentUser: root.getPropertyValue('--accent-user').trim(),
		background: body.backgroundColor,
		text: body.color,
		cardRadius: card ? card.borderRadius : null,
		cardBackground: card ? card.backgroundColor : null,
		cardBorder: card ? card.borderTopColor : null,
		primaryButton: button ? button.backgroundColor : null,
		activeNav: active ? getComputedStyle(active).color : null,
		stylesheets: document.styleSheets.length,
		missing,
	});
})()`;

const answer = await evaluate(probe);
const value = answer.result?.result?.value;

if (!value) {
	console.error(JSON.stringify(answer, null, 2));
	process.exit(1);
}

console.log(value);
socket.close();
