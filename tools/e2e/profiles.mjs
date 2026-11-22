// Checks the named profiles: they are listed, chosen, created, edited and deleted from the
// interface, and the core resolves the profile of the selected account (version, loader, memory,
// window) from then on. Editing one profile must never touch the others nor the flat settings.
//
// run-all.sh seeds the home with a second profile, `Hardcore`.
//
// Usage: node profiles.mjs <debugging-port>

import { setTimeout as sleep } from 'node:timers/promises';
import { createClick } from './click.mjs';
import { requireTestHome, requireTestPaths } from './guard.mjs';

// Never on the profile of the player who runs the check.
requireTestHome();


const port = process.argv[2] || '9222';
const results = [];

function record(name, ok, detail = '') {
	results.push({ name, ok });
	console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
}

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

async function waitFor(expression, description, timeout = 20000) {
	const start = Date.now();
	while (Date.now() - start < timeout) {
		if (await evaluate(expression)) return true;
		await sleep(200);
	}
	throw new Error(`timeout while waiting for ${description}`);
}

/** A real mouse click in the middle of an element, brought into view and checked first. */
const { click } = createClick({ evaluate, send }, { settle: 150, move: false });

/** Clicks a field and types into it, through the browser's own input. */
async function type(selector, text) {
	await click(selector);
	await send('Input.insertText', { text });
}

/** Chooses a value in a native select (its dropdown is not reachable over CDP). */
async function choose(selector, value) {
	return evaluate(`(() => {
		const select = document.querySelector(${JSON.stringify(selector)});
		select.value = ${JSON.stringify(value)};
		select.dispatchEvent(new Event('change', { bubbles: true }));
		return select.value;
	})()`);
}

/** Moves a slider the way a mouse does: input, then change when the thumb is released. */
async function setRange(selector, value) {
	return evaluate(`(() => {
		const input = document.querySelector(${JSON.stringify(selector)});
		input.value = ${Number(value)};
		input.dispatchEvent(new Event('input', { bubbles: true }));
		input.dispatchEvent(new Event('change', { bubbles: true }));
		return input.value;
	})()`);
}

/** The delete button lives on the Instances page, next to the profile catalog. */
async function openInstancesPage() {
	await click('.nav-item[data-page="versions"]');
	await waitFor(`document.querySelector('.page.is-active')?.dataset.page === 'versions'`, 'the instances page');
}

const options = () => evaluate(
	`Array.from(document.querySelectorAll('#select-profile option')).map((option) => option.value)`
);

const profileNamed = (name) => evaluate(
	`window.aether.profiles.list().then((list) => list.profiles.find((profile) => profile.name === ${JSON.stringify(name)}) || null)`
);

await send('Runtime.enable');
await waitFor(`document.querySelector('#paths-output').textContent.trim().length > 0`, 'the initial state');
await sleep(500);

// --- create an offline account through the real UI --------------------------------------------
let accountStore = await evaluate('window.aether.accounts.get()');
record('the account store is reachable through the launcher bridge', accountStore?.ok === true, accountStore?.error || '');

if (accountStore?.ok && accountStore.accounts.length === 0) {
	await click('.nav-item[data-page="accounts"]');
	await waitFor(`document.querySelector('.page.is-active')?.dataset.page === 'accounts'`, 'the accounts page');
	await click('#btn-add-offline');
	await waitFor(`document.querySelector('#modal').hidden === false`, 'the offline account dialog');
	await type('#offline-name', 'Notch');
	await click('#modal-confirm');
	await waitFor(
		`window.aether.accounts.get().then((store) => store.ok && store.accounts.length === 1 && Boolean(store.selected))`,
		'the offline account to be saved and selected'
	);
	accountStore = await evaluate('window.aether.accounts.get()');
}

record('an offline account can be created and selected from the interface',
	accountStore?.ok === true && accountStore.accounts.length === 1
		&& accountStore.accounts[0].username === 'Notch'
		&& accountStore.selected === accountStore.accounts[0].id,
	JSON.stringify({ accounts: accountStore?.accounts?.map((account) => account.username), selected: accountStore?.selected }));

await click('.nav-item[data-page="play"]');
await waitFor(`document.querySelector('.page.is-active')?.dataset.page === 'play'`, 'the play page');

// --- the catalog ------------------------------------------------------------------------------
const catalog = await evaluate('window.aether.profiles.list()');
record('the core lists the profiles of the player',
	catalog.ok && catalog.profiles.length === 2 && catalog.active === 'Default',
	JSON.stringify({ names: (catalog.profiles || []).map((profile) => profile.name), active: catalog.active }));

record('the interface offers them', (await options()).length === 2, JSON.stringify(await options()));

const before = await evaluate('window.aether.info()');

// The window says where it really writes: a home outside tools/ is a real profile.
requireTestPaths(before);
record('the account starts on the default profile',
	before.profile.name === 'Default' && before.profile.maxMemory === 4096,
	JSON.stringify(before.profile));

// --- assign the profile of the account --------------------------------------------------------
await choose('#select-profile', 'Hardcore');
await waitFor(
	`window.aether.info().then((info) => info.profile && info.profile.name === 'Hardcore')`,
	'the profile to reach the core'
);

const after = await evaluate('window.aether.info()');
record('the account plays with that profile',
	after.profile.gameVersion === '1.20.1'
		&& after.profile.loaderVersion === '0.20.1'
		&& after.profile.maxMemory === 8192,
	JSON.stringify(after.profile));

const store = await evaluate('window.aether.accounts.get()');
record('the choice is stored on the account',
	store.accounts[0].profile === 'Hardcore',
	JSON.stringify(store.accounts[0].profile));

record('the interface shows the profile that applies',
	(await evaluate(`document.querySelector('#select-profile').value`)) === 'Hardcore');

// --- the play controls edit that profile ------------------------------------------------------
const flatBefore = (await evaluate('window.aether.settings.get()')).settings;

await setRange('#range-memory', 10240);
await waitFor(
	`window.aether.profiles.list().then((list) => list.profiles.find((profile) => profile.name === 'Hardcore').maxMemory === 10240)`,
	'the memory of the profile'
);

record('the memory control edits the selected profile',
	(await profileNamed('Hardcore')).maxMemory === 10240,
	JSON.stringify((await profileNamed('Hardcore')).maxMemory));
record('the other profiles keep their memory',
	(await profileNamed('Default')).maxMemory === 4096,
	JSON.stringify((await profileNamed('Default')).maxMemory));
record('the flat settings are left alone',
	(await evaluate('window.aether.settings.get()')).settings.maxMemory === flatBefore.maxMemory,
	`${(await evaluate('window.aether.settings.get()')).settings.maxMemory}`);

await setRange('#range-width', 1280);
await waitFor(
	`window.aether.profiles.list().then((list) => list.profiles.find((profile) => profile.name === 'Hardcore').windowWidth === 1280)`,
	'the window width of the profile'
);

record('the window control edits the selected profile',
	(await profileNamed('Hardcore')).windowWidth === 1280,
	JSON.stringify((await profileNamed('Hardcore')).windowWidth));
record('the other profiles keep their window size',
	(await profileNamed('Default')).windowWidth === 854,
	JSON.stringify((await profileNamed('Default')).windowWidth));

const playedWith = await evaluate('window.aether.info()');
record('the account plays with the edited profile',
	playedWith.profile.maxMemory === 10240 && playedWith.profile.windowWidth === 1280,
	JSON.stringify({ memory: playedWith.profile.maxMemory, width: playedWith.profile.windowWidth }));

// The version list comes from Mojang; the check uses whatever second version it offers.
const otherVersion = await evaluate(`(() => {
	const select = document.querySelector('#select-version');
	const option = Array.from(select.options).find((entry) => entry.value !== select.value);
	return option ? option.value : null;
})()`);

if (otherVersion) {
	await choose('#select-version', otherVersion);
	await waitFor(
		`window.aether.profiles.list().then((list) => list.profiles.find((profile) => profile.name === 'Hardcore').gameVersion === ${JSON.stringify(otherVersion)})`,
		'the version of the profile'
	);

	record('the version control edits the selected profile',
		(await profileNamed('Hardcore')).gameVersion === otherVersion,
		otherVersion);
	record('the flat settings keep their version',
		(await evaluate('window.aether.settings.get()')).settings.gameVersion === flatBefore.gameVersion,
		flatBefore.gameVersion);
} else {
	record('the version control edits the selected profile', true, 'skipped, a single version is listed');
}

// --- create a profile -------------------------------------------------------------------------
await click('#btn-profile-new');
await waitFor(`document.querySelector('#modal').hidden === false`, 'the new profile dialog');
await type('#instance-name', 'Cinematic');
await click('#modal-confirm');
await waitFor(
	`window.aether.profiles.list().then((list) => list.profiles.length === 3)`,
	'the new profile'
);

const cinematic = await profileNamed('Cinematic');
record('a profile is created from the interface',
	cinematic !== null && (await options()).includes('Cinematic'),
	JSON.stringify(cinematic));

record('the new profile copies the one that was playing',
	cinematic.maxMemory === 10240 && cinematic.windowWidth === 1280,
	JSON.stringify({ memory: cinematic.maxMemory, width: cinematic.windowWidth }));

record('the dialog closes once the profile exists',
	(await evaluate(`document.querySelector('#modal').hidden`)) === true);

await choose('#select-profile', 'Cinematic');
await waitFor(
	`window.aether.info().then((info) => info.profile.name === 'Cinematic')`,
	'the new profile to apply'
);
record('the new profile can be played', true);

// --- delete a profile, with a confirmation ----------------------------------------------------
await openInstancesPage();
await click('#btn-profile-delete');
await waitFor(`document.querySelector('#modal').hidden === false`, 'the delete dialog');

const dialog = await evaluate(`(() => ({
	title: document.querySelector('#modal-title').textContent,
	name: document.querySelector('#profile-to-delete').textContent,
	confirm: document.querySelector('#modal-confirm').textContent
}))()`);
record('the deletion asks for a confirmation first',
	dialog.title === "Supprimer l'instance" && dialog.name === 'Cinematic' && dialog.confirm === 'Supprimer',
	JSON.stringify(dialog));
record('nothing is deleted while the dialog is open',
	(await profileNamed('Cinematic')) !== null);

await click('#modal-cancel');
await sleep(300);
record('the cancel keeps the profile',
	(await evaluate(`document.querySelector('#modal').hidden`)) === true && (await profileNamed('Cinematic')) !== null);

await openInstancesPage();
await click('#btn-profile-delete');
await waitFor(`document.querySelector('#modal').hidden === false`, 'the delete dialog again');
await click('#modal-confirm');
await waitFor(
	`window.aether.profiles.list().then((list) => list.profiles.length === 2)`,
	'the profile to be removed'
);

const removed = await evaluate('window.aether.profiles.list()');
const back = await evaluate('window.aether.accounts.get()');
const resolved = await evaluate('window.aether.info()');
record('a profile is deleted once confirmed',
	removed.profiles.every((profile) => profile.name !== 'Cinematic')
		&& !(await options()).includes('Cinematic'),
	JSON.stringify(removed.profiles.map((profile) => profile.name)));

record('the account falls back to the active profile after the deletion',
	resolved.profile.name === 'Default' && !back.accounts[0].profile,
	JSON.stringify({ profile: resolved.profile.name, stored: back.accounts[0].profile }));

// --- the default profile is protected ---------------------------------------------------------
await openInstancesPage();
await click('#btn-profile-delete');
await waitFor(`document.querySelector('#modal').hidden === false`, 'the delete dialog of the default profile');
await click('#modal-confirm');
await waitFor(
	`document.querySelector('#toasts').textContent.includes('ne peut pas être supprimée')`,
	'the refusal of the core'
);

const refused = await evaluate('window.aether.profiles.list()');
record('the default profile cannot be deleted',
	refused.profiles.length === 2 && refused.profiles.some((profile) => profile.name === 'Default'),
	JSON.stringify(refused.profiles.map((profile) => profile.name)));

socket.close();
await sleep(200);
const failed = results.filter((entry) => !entry.ok).length;
console.log(`SUMMARY ${results.length - failed}/${results.length} checks passed`);
process.exit(failed === 0 ? 0 : 1);
