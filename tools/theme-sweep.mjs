// Walks every page of the real launcher window and audits what the engine actually renders.
//
// Usage:
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9247" aether-desktop.exe
//   node tools/theme-sweep.mjs 9247 tools/theme-pages
//
// What it does, in the window and not in the abstract:
//  1. sweeps the eight pages, screenshots each one and audits it:
//       - every `var(--…)` of every loaded stylesheet resolves on :root,
//       - every visible run of text has a contrast ratio above 4.5 (3 for large text),
//       - no element still wears a colour of the old palette;
//  2. clicks the Settings entry, clicks the accent swatch, and drives the `input`/`change` pair the
//     native colour dialog emits on `#input-accent` — `applyAccent` is exercised, `--accent` is never
//     written by this script;
//  3. reads the computed value of every accent-derived token before and after, then sweeps again.

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const port = process.argv[2] || '9247';
const outDir = process.argv[3] || 'tools/theme-pages';
const target = process.argv[4] || null; // the accent to pick through the control

const PAGES = ['play', 'accounts', 'versions', 'mods', 'servers', 'performance', 'logs', 'settings'];
const PICK = target || '#560b0b';

await mkdir(outDir, { recursive: true });

/* ------------------------------------------------------------------ the protocol */

async function findPage() {
	for (let attempt = 0; attempt < 80; attempt++) {
		try {
			const answer = await fetch(`http://127.0.0.1:${port}/json/list`);
			const targets = await answer.json();
			const page = targets.find((entry) => entry.type === 'page' && entry.webSocketDebuggerUrl);
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

function send(method, params = {}) {
	const id = nextId++;
	socket.send(JSON.stringify({ id, method, params }));
	return new Promise((resolve) => pending.set(id, resolve));
}

async function evaluate(expression) {
	const answer = await send('Runtime.evaluate', {
		expression,
		returnByValue: true,
		awaitPromise: true,
		userGesture: true,
	});

	const exception = answer.result?.exceptionDetails;
	if (exception) throw new Error(exception.text || 'the page threw');

	return answer.result?.result?.value;
}

await send('Page.enable');
await send('Runtime.enable');

/* ------------------------------------------------------------------- the gestures */

/** Clicks the centre of an element with real mouse events. */
async function click(selector) {
	const box = await evaluate(`(() => {
		const element = document.querySelector(${JSON.stringify(selector)});
		if (!element) return null;
		const rect = element.getBoundingClientRect();
		return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
	})()`);

	if (!box) throw new Error(`no element ${selector}`);

	for (const type of ['mousePressed', 'mouseReleased']) {
		await send('Input.dispatchMouseEvent', {
			type,
			x: box.x,
			y: box.y,
			button: 'left',
			clickCount: 1,
		});
	}

	await sleep(120);
}

/** Opens a page by clicking its entry in the sidebar, the way a player does. */
async function openPage(page) {
	await click(`.nav-item[data-page="${page}"]`);
	await sleep(450);
}

/** Hovers a text field with the mouse and reads the border it takes: that is `--border-hover`. */
async function hoverField() {
	const box = await evaluate(`(() => {
		const element = document.querySelector('#input-java');
		if (!element) return null;
		const rect = element.getBoundingClientRect();
		return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
	})()`);

	if (!box) return null;

	await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: box.x, y: box.y });
	await sleep(300);

	return evaluate(`(() => {
		const computed = getComputedStyle(document.querySelector('#input-java'));
		return { border: computed.borderTopColor };
	})()`);
}

/* -------------------------------------------------------------------- the reading */

const AUDIT = String.raw`(() => {
	const style = getComputedStyle(document.documentElement);
	const sheets = [...document.styleSheets];

	/* 1. every var(--…) the stylesheets mention must resolve on :root */
	const referenced = new Set();
	for (const sheet of sheets) {
		let rules;
		try { rules = sheet.cssRules; } catch { continue; }
		const walk = (list) => {
			for (const rule of list) {
				if (rule.cssRules) walk(rule.cssRules);
				if (!rule.style) continue;
				for (const property of rule.style) {
					const value = rule.style.getPropertyValue(property);
					for (const match of value.matchAll(/var\(\s*(--[a-z0-9-]+)/gi)) referenced.add(match[1]);
				}
			}
		};
		walk(rules);
	}
	const unresolved = [];
	for (const name of referenced) {
		if (style.getPropertyValue(name).trim() === '') unresolved.push(name);
	}

	/* 2. colours: rgb(), rgba() and the color(srgb …) that color-mix() produces */
	const parse = (value) => {
		const rgb = value.match(/rgba?\(([^)]+)\)/);
		if (rgb) {
			const parts = rgb[1].split(/[\s,\/]+/).filter(Boolean).map(Number);
			return { r: parts[0], g: parts[1], b: parts[2], a: parts.length > 3 ? parts[3] : 1 };
		}
		const srgb = value.match(/color\(srgb\s+([^)]+)\)/);
		if (srgb) {
			const parts = srgb[1].split(/[\s\/]+/).filter(Boolean).map(Number);
			return { r: parts[0] * 255, g: parts[1] * 255, b: parts[2] * 255, a: parts.length > 3 ? parts[3] : 1 };
		}
		return null;
	};

	const over = (top, bottom) => ({
		r: top.r * top.a + bottom.r * (1 - top.a),
		g: top.g * top.a + bottom.g * (1 - top.a),
		b: top.b * top.a + bottom.b * (1 - top.a),
		a: 1,
	});

	const luminance = (colour) => {
		const channel = (value) => {
			const v = value / 255;
			return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
		};
		return 0.2126 * channel(colour.r) + 0.7152 * channel(colour.g) + 0.0722 * channel(colour.b);
	};

	const ratio = (a, b) => {
		const la = luminance(a);
		const lb = luminance(b);
		return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
	};

	const background = (element) => {
		const stack = [];
		let current = element;
		while (current && current.nodeType === 1) {
			const colour = parse(getComputedStyle(current).backgroundColor);
			if (colour && colour.a > 0.001) {
				stack.push(colour);
				if (colour.a >= 0.999) break;
			}
			current = current.parentElement;
		}

		let result = { r: 7, g: 11, b: 18, a: 1 };
		for (let index = stack.length - 1; index >= 0; index--) result = over(stack[index], result);
		return result;
	};

	const path = (element) => {
		const parts = [];
		let current = element;
		while (current && current.nodeType === 1 && parts.length < 4) {
			const name = current.tagName.toLowerCase();
			const classes = (current.className || '').toString().trim().split(/\s+/).filter(Boolean).slice(0, 2);
			parts.unshift(name + (classes.length ? '.' + classes.join('.') : ''));
			current = current.parentElement;
		}
		return parts.join(' > ');
	};

	const problems = [];
	let measured = 0;

	for (const element of document.querySelectorAll('body *')) {
		const own = [...element.childNodes].some((node) => node.nodeType === 3 && node.textContent.trim().length > 0);
		if (!own) continue;

		const box = element.getBoundingClientRect();
		const computed = getComputedStyle(element);
		if (box.width < 1 || box.height < 1) continue;
		if (computed.visibility === 'hidden' || computed.display === 'none' || Number(computed.opacity) < 0.5) continue;

		const foreground = parse(computed.color);
		if (!foreground) continue;

		const behind = background(element);
		const value = ratio(over(foreground, behind), behind);
		const size = parseFloat(computed.fontSize);
		const large = size >= 24 || (size >= 18.66 && Number(computed.fontWeight) >= 600);
		const floor = large ? 3 : 4.5;

		measured++;
		if (value < floor) {
			problems.push({
				kind: 'contrast',
				where: path(element),
				text: element.textContent.trim().slice(0, 40),
				colour: computed.color,
				behind: 'rgb(' + [behind.r, behind.g, behind.b].map((n) => Math.round(n)).join(',') + ')',
				ratio: Number(value.toFixed(2)),
				floor,
			});
		}
	}

	/* 3. the palette of the previous theme must not survive anywhere */
	const LEGACY = [[124, 92, 252], [56, 189, 248], [196, 181, 253], [108, 92, 231], [11, 14, 20], [20, 24, 31], [35, 43, 56]];
	const survivors = [];

	for (const element of document.querySelectorAll('body *')) {
		const computed = getComputedStyle(element);
		for (const property of ['color', 'backgroundColor', 'borderTopColor', 'boxShadow', 'fill']) {
			const parsed = parse(computed[property]);
			if (!parsed) continue;
			const match = LEGACY.find(
				(entry) =>
					Math.abs(entry[0] - parsed.r) < 2 && Math.abs(entry[1] - parsed.g) < 2 && Math.abs(entry[2] - parsed.b) < 2,
			);
			if (match) {
				survivors.push({ kind: 'legacy', where: path(element), property, colour: computed[property] });
				break;
			}
		}
	}

	/* 4. the elements that consume each derived token, read as resolved colours */
	const pick = (selector) => {
		const element = document.querySelector(selector);
		if (!element) return null;
		const computed = getComputedStyle(element);
		return {
			color: computed.color,
			background: computed.backgroundColor,
			border: computed.borderTopColor,
		};
	};

	const consumers = {};
	for (const [name, selector] of Object.entries({
		primaryButton: '.btn--primary',
		rangeValue: '.range-row output',
		badge: '.badge, .pill, [class*="badge"]',
	})) {
		const value = pick(selector);
		if (value) consumers[name] = value;
	}

	return JSON.stringify({
		unresolved,
		measured,
		problems,
		survivors,
		consumers,
		tokens: {
			accent: style.getPropertyValue('--accent').trim(),
			'accent-dim': style.getPropertyValue('--accent-dim').trim(),
			'accent-soft': style.getPropertyValue('--accent-soft').trim(),
			'accent-mid': style.getPropertyValue('--accent-mid').trim(),
			'accent-text': style.getPropertyValue('--accent-text').trim(),
			'accent-hover': style.getPropertyValue('--accent-hover').trim(),
			'border-hover': style.getPropertyValue('--border-hover').trim(),
		},
	});
})()`;

async function audit() {
	return JSON.parse(await evaluate(AUDIT));
}

async function screenshot(name) {
	const answer = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
	const data = answer.result?.data;
	if (!data) throw new Error('the screenshot was refused');
	await writeFile(join(outDir, `${name}.png`), Buffer.from(data, 'base64'));
}

/* ---------------------------------------------------------------------- the sweep */

const report = { pages: [], picked: PICK, after: null };

await sleep(1200);

const sweep = async (label) => {
	const pages = {};
	for (const page of PAGES) {
		await openPage(page);
		pages[page] = await audit();
		await screenshot(`${label}-${page}`);
	}
	return pages;
};

report.pages.push({ accent: 'defaut', data: await sweep('defaut') });

// The hover state of a text field consumes `--border-hover`, read with a real mouse move.
await openPage('settings');
report.hoverBefore = await hoverField();

// The accent control, through the window: Settings, then the swatch, then the event pair the native
// dialog emits. Nothing here writes `--accent` or `--accent-user`.
await openPage('settings');
await click('#input-accent');
await sleep(500);
for (const type of ['keyDown', 'keyUp']) {
	await send('Input.dispatchKeyEvent', { type, key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 });
}
await sleep(300);

const picked = await evaluate(`(() => {
	const input = document.querySelector('#input-accent');
	const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
	setter.call(input, ${JSON.stringify(PICK)});
	input.dispatchEvent(new Event('input', { bubbles: true }));
	input.dispatchEvent(new Event('change', { bubbles: true }));
	return { value: input.value, label: document.querySelector('#output-accent').textContent };
})()`);

report.pickedControl = picked;
await sleep(900);
report.after = await audit();
report.hoverAfter = await hoverField();

report.pages.push({ accent: 'choisi', data: await sweep('choisi') });

await writeFile(join(outDir, 'report.json'), JSON.stringify(report, null, 2));

const summarise = (label, pages) => {
	for (const [page, data] of Object.entries(pages)) {
		console.log(
			`${label.padEnd(7)} ${page.padEnd(12)} accent=${data.tokens.accent} unresolved=${data.unresolved.length} ` +
				`contraste=${data.problems.length} litteraux=${data.survivors.length} mesures=${data.measured}`,
		);
	}
};

console.log(`avant: ${JSON.stringify(report.pages[0].data.play.tokens)}`);
console.log(`controle: ${JSON.stringify(picked)}`);
console.log(`apres: ${JSON.stringify(report.after.tokens)}`);
summarise('defaut', report.pages[0].data);
summarise('choisi', report.pages[1].data);

socket.close();
