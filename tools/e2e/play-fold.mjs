// Measures the action row of the Play page in the real window: the primary action has to be clickable
// without scrolling, so the install and cancel buttons must sit entirely inside the viewport.
//
// Usage: node tools/e2e/play-fold.mjs <debugging-port> [--install] [--size <width>x<height>] [--json <path>]
//
// `--install` clicks Install / repair first (needs the fake core with AETHER_FAKE_FAIL=install-slow)
// and measures the row with Cancel install on screen, the state the theme sweep audits.
// `--size` overrides the layout viewport for the run: 980x700 and 1180x760 are the sizes the page has
// to hold, the second one being the window the shell opens with.

import { writeFileSync } from 'node:fs';
import { requireTestHome } from './guard.mjs';

requireTestHome();

const port = process.argv[2] || '9222';
const install = process.argv.includes('--install');
const outputIndex = process.argv.indexOf('--json');
const output = outputIndex > 0 ? process.argv[outputIndex + 1] : null;
const sizeIndex = process.argv.indexOf('--size');
const size = sizeIndex > 0 ? String(process.argv[sizeIndex + 1]).split('x').map(Number) : null;
const shotIndex = process.argv.indexOf('--shot');
const shot = shotIndex > 0 ? process.argv[shotIndex + 1] : null;

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function connect() {
	const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
	const page = targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
	if (!page) throw new Error('no page on this port');

	const socket = new WebSocket(page.webSocketDebuggerUrl);
	await new Promise((resolve, reject) => {
		socket.onopen = resolve;
		socket.onerror = () => reject(new Error('the page refused the connection'));
	});

	let nextId = 1;
	const pending = new Map();
	socket.onmessage = (event) => {
		const message = JSON.parse(event.data);
		if (pending.has(message.id)) pending.get(message.id)(message);
	};

	const send = (method, params) =>
		new Promise((resolve, reject) => {
			const id = nextId++;
			pending.set(id, (message) => {
				const exception = message.result?.exceptionDetails;
				if (exception) reject(new Error(exception.exception?.description || exception.text));
				else resolve(message.result?.result?.value ?? message.result);
			});
			socket.send(JSON.stringify({ id, method, params }));
		});

	const evaluate = (expression) =>
		send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: true });

	return { socket, send, evaluate };
}

const measure = `(() => {
	const rect = (selector) => {
		const element = document.querySelector(selector);
		if (!element) return null;
		const box = element.getBoundingClientRect();
		const style = getComputedStyle(element);
		const shown = style.display !== 'none' && style.visibility !== 'hidden';
		return { top: +box.top.toFixed(2), bottom: +box.bottom.toFixed(2), height: +box.height.toFixed(2), visible: shown && box.height > 0 };
	};
	const inside = (box) => !!box && box.visible && box.top >= 0 && box.bottom <= innerHeight;

	// The scroll zone has to be worth scrolling: tall enough to read a card head, and able to reach
	// the bottom of the cards it holds.
	const zone = document.querySelector('.play-scroll');
	const grid = document.querySelector('.grid--two');
	let scrollZone = null;
	if (zone && grid) {
		const start = zone.scrollTop;
		zone.scrollTop = zone.scrollHeight;
		const zoneBox = zone.getBoundingClientRect();
		const gridBox = grid.getBoundingClientRect();
		zone.focus();
		scrollZone = {
			height: zone.clientHeight,
			content: zone.scrollHeight,
			scrollable: zone.scrollHeight > zone.clientHeight + 1,
			reachedBottom: gridBox.bottom <= zoneBox.bottom + 1,
			overflowY: getComputedStyle(zone).overflowY,
			tabindex: zone.getAttribute('tabindex'),
			role: zone.getAttribute('role'),
			label: zone.getAttribute('aria-label'),
			focusable: document.activeElement === zone
		};
		zone.blur();
		zone.scrollTop = start;
	}

	const install = rect('#btn-install');
	const cancel = rect('#btn-cancel-install');
	const contentElement = document.querySelector('.content');
	const failures = [['#btn-install', install], ['#btn-cancel-install', cancel]]
		.filter(([, box]) => box && box.visible && !inside(box))
		.map(([name]) => name);
	if (scrollZone && scrollZone.height < 140) failures.push('zone defilante trop courte');
	if (scrollZone && !scrollZone.reachedBottom) failures.push('zone defilante inutilisable');
	if (scrollZone && !scrollZone.focusable) failures.push('zone defilante hors du clavier');
	if (scrollZone && (!scrollZone.role || !scrollZone.label)) failures.push('zone defilante sans nom accessible');
	if (contentElement && contentElement.scrollHeight > contentElement.clientHeight + 2) failures.push('la page defile encore');

	return {
		viewport: { width: innerWidth, height: innerHeight },
		scrollTop: document.scrollingElement.scrollTop,
		content: rect('.content'),
		hero: rect('.hero-banner'),
		grid: rect('.grid--two'),
		session: rect('.card--hero-launch'),
		actions: rect('.card--hero-launch .actions'),
		console: rect('#console'),
		progress: rect('#install-progress'),
		install,
		launch: rect('#btn-launch'),
		cancel,
		scrollZone,
		installInside: inside(install),
		cancelInside: cancel && cancel.visible ? inside(cancel) : null,
		failures
	};
})()`;

const { socket, send, evaluate } = await connect();

if (size) {
	if (size.length !== 2 || size.some((value) => !Number.isFinite(value))) throw new Error('--size wants <width>x<height>');
	await send('Emulation.setDeviceMetricsOverride', { width: size[0], height: size[1], deviceScaleFactor: 1, mobile: false });
}

// A scroll position left by an earlier check would hide the defect: every measurement starts at the top.
await evaluate('document.scrollingElement.scrollTop = 0; 1');
await wait(150);

if (install) {
	const clicked = await evaluate(
		`(() => { const button = document.querySelector('#btn-install'); if (!button || button.disabled) return 'unavailable'; button.click(); return 'clicked'; })()`
	);
	if (clicked !== 'clicked') throw new Error(`the install button is ${clicked}`);
	for (let attempt = 0; attempt < 40; attempt++) {
		await wait(250);
		const ready = await evaluate(
			`(() => { const button = document.querySelector('#btn-cancel-install'); return !!button && !button.hidden && button.offsetHeight > 0; })()`
		);
		if (ready) break;
		if (attempt === 39) throw new Error('the install state never showed cancel');
	}
	await evaluate('document.scrollingElement.scrollTop = 0; 1');
	await wait(150);
}

const result = await evaluate(measure);

// A screenshot of the measured state, so the geometry can be checked against what is on screen.
if (shot) {
	await send('Page.enable', {});
	const captured = await send('Page.captureScreenshot', { format: 'png' });
	writeFileSync(shot, Buffer.from(captured.data, 'base64'));
	console.log(`capture: ${shot}`);
}

if (size) await send('Emulation.clearDeviceMetricsOverride', {});
socket.close();

result.state = install ? 'install' : 'idle';
if (output) writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`);

for (const [name, box] of [['#btn-install', result.install], ['#btn-cancel-install', result.cancel]]) {
	if (!box) continue;
	const verdict = box.visible ? (result.failures.includes(name) ? 'HORS FENETRE' : 'dans la fenetre') : 'cache';
	console.log(`${name} top=${box.top} bottom=${box.bottom} hauteur=${box.height} -> ${verdict}`);
}
console.log(
	`viewport ${result.viewport.width}x${result.viewport.height} | hero ${result.hero?.height} | grille ${result.grid?.height} | session ${result.session?.height} | actions ${result.actions?.top}-${result.actions?.bottom} | console ${result.console?.height}`
);
if (result.scrollZone) {
	const zone = result.scrollZone;
	console.log(
		`zone defilante hauteur=${zone.height} contenu=${zone.content} defile=${zone.scrollable} atteint_le_bas=${zone.reachedBottom} focus=${zone.focusable} role=${zone.role} nom="${zone.label}"`
	);
}
console.log(result.failures.length === 0 ? 'PASS action principale entierement visible' : `FAIL ${result.failures.join(', ')}`);
if (output) console.log(`rapport: ${output}`);
process.exit(result.failures.length === 0 ? 0 : 1);
