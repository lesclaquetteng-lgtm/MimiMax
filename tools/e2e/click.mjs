// A real mouse click for the e2e checks.
//
// The target is scrolled to the middle of the page, then the point that will be clicked is checked
// against `document.elementFromPoint` before any event is sent. A click that would land on a toast,
// a dialog backdrop or a hidden layer therefore fails at once, with the name of what is really
// there, instead of clicking nothing and waiting for the check's 30 s timeout.
//
// Usage:
//   import { createClick, pointAt } from './click.mjs';
//   const { aim, click } = createClick({ evaluate, send }, { settle: 150 });
//   await click('#btn-install');

import { setTimeout as sleep } from 'node:timers/promises';

/** Names the node in the way the way a developer would say it. */
const describe = `(node) => node
	? (node.id
		? '#' + node.id
		: (typeof node.className === 'string' && node.className.trim()
			? '.' + node.className.trim().split(/\\s+/).join('.')
			: node.tagName.toLowerCase()))
	: 'nothing'`;

/**
 * The point at `fraction` of the width of `selector`, centred vertically, checked against what the
 * mouse would really hit. A missing, hidden, disabled, off-screen or covered element throws here.
 */
export async function pointAt(evaluate, selector, fraction = 0.5) {
	await evaluate(`(() => {
		const element = document.querySelector(${JSON.stringify(selector)});
		if (element) element.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'instant' });
		return true;
	})()`);
	await sleep(50);

	const point = await evaluate(`(() => {
		const element = document.querySelector(${JSON.stringify(selector)});
		const describe = ${describe};
		if (!element) return { x: 0, y: 0, box: null, hit: '', reachable: false, problem: 'is not in the page' };
		const rect = element.getBoundingClientRect();
		const x = rect.x + rect.width * ${fraction};
		const y = rect.y + rect.height / 2;
		const onScreen = x >= 0 && y >= 0 && x <= innerWidth && y <= innerHeight;
		const at = onScreen ? document.elementFromPoint(x, y) : null;
		const reachable = !!at && (at === element || element.contains(at));
		return {
			x,
			y,
			box: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			hit: describe(at),
			reachable,
			hidden: element.hidden === true,
			disabled: element.disabled === true,
			problem: reachable
				? ''
				: onScreen
					? 'is covered by ' + describe(at) + ' at ' + Math.round(x) + ',' + Math.round(y)
					: 'is outside the window at ' + Math.round(x) + ',' + Math.round(y)
		};
	})()`);

	if (point.problem === 'is not in the page') throw new Error(`${selector} ${point.problem}`);
	if (point.hidden) throw new Error(`${selector} is hidden, it cannot be clicked`);
	if (point.disabled) throw new Error(`${selector} is disabled, it cannot be clicked`);
	if (!point.reachable) throw new Error(`${selector} ${point.problem}`);
	return point;
}

/**
 * Binds the click helper of a check to its DevTools connection: `aim` returns the checked point
 * without sending anything, `click` sends the real mouse events and returns that point.
 */
export function createClick({ evaluate, send }, { settle = 150, move = true } = {}) {
	async function click(selector) {
		const point = await pointAt(evaluate, selector);
		const types = move ? ['mouseMoved', 'mousePressed', 'mouseReleased'] : ['mousePressed', 'mouseReleased'];

		for (const type of types) {
			await send('Input.dispatchMouseEvent', {
				type,
				x: point.x,
				y: point.y,
				button: 'left',
				buttons: type === 'mousePressed' ? 1 : 0,
				clickCount: 1
			});
		}
		await sleep(settle);
		return point;
	}

	return { aim: (selector) => pointAt(evaluate, selector), click };
}
