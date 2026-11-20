const api = window.aether;

const state = {
	page: 'play',
	accounts: { accounts: [], selected: null },
	settings: {},
	versions: [],
	loaders: [],
	mods: [],
	profiles: { profiles: [], activeFor: null },
	servers: { servers: [], pings: {} },
	hosts: [],
	performance: { mods: [], state: {}, installing: false },
	sessionLines: 0,
	installing: false,
	launching: false
};

// The state of the interface, read by the end to end check to see the flags go back to idle.
window.aetherState = state;

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => Array.from(document.querySelectorAll(selector));

/* ------------------------------------------------------------------ helpers */

function toast(message, kind = '') {
	const element = document.createElement('div');
	element.className = `toast ${kind ? `toast--${kind}` : ''}`;
	element.textContent = message;
	// An error is announced at once, the other messages wait for a pause.
	if (kind === 'error') element.setAttribute('role', 'alert');
	$('#toasts').appendChild(element);
	setTimeout(() => {
		element.style.opacity = '0';
		element.style.transition = 'opacity .25s';
		setTimeout(() => element.remove(), 260);
	}, 4200);
}

function setStatus(text, kind = '') {
	const pill = $('#status-pill');
	pill.className = `status-pill${kind ? ` is-${kind}` : ''}`;
	$('#status-text').textContent = text;
}

/** Parses Minecraft MOTD formatting codes (§0-f, §l,m,n,o,r) into styled sanitized HTML spans. */
function parseMinecraftMotd(raw) {
	if (!raw) return '<span class="muted">No description provided</span>';
	const colorMap = {
		'0': 'mc-0', '1': 'mc-1', '2': 'mc-2', '3': 'mc-3',
		'4': 'mc-4', '5': 'mc-5', '6': 'mc-6', '7': 'mc-7',
		'8': 'mc-8', '9': 'mc-9', 'a': 'mc-a', 'b': 'mc-b',
		'c': 'mc-c', 'd': 'mc-d', 'e': 'mc-e', 'f': 'mc-f'
	};
	const styleMap = {
		'l': 'mc-bold', 'm': 'mc-strikethrough', 'n': 'mc-underlined', 'o': 'mc-italic'
	};

	let currentColor = '';
	let currentStyles = new Set();
	let html = '';
	let buffer = '';

	const flushBuffer = () => {
		if (!buffer) return;
		const escaped = buffer
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;');
		const classes = [];
		if (currentColor) classes.push(currentColor);
		currentStyles.forEach((s) => classes.push(s));

		if (classes.length > 0) {
			html += `<span class="${classes.join(' ')}">${escaped}</span>`;
		} else {
			html += escaped;
		}
		buffer = '';
	};

	for (let i = 0; i < raw.length; i++) {
		const char = raw[i];
		if ((char === '§' || char === '&') && i + 1 < raw.length) {
			const code = raw[i + 1].toLowerCase();
			if (colorMap[code]) {
				flushBuffer();
				currentColor = colorMap[code];
				currentStyles.clear();
				i++;
				continue;
			} else if (styleMap[code]) {
				flushBuffer();
				currentStyles.add(styleMap[code]);
				i++;
				continue;
			} else if (code === 'r') {
				flushBuffer();
				currentColor = '';
				currentStyles.clear();
				i++;
				continue;
			}
		}

		if (char === '\n') {
			flushBuffer();
			html += '<br>';
		} else {
			buffer += char;
		}
	}

	flushBuffer();
	return html || '<span class="muted">A Minecraft Server</span>';
}

/* -------------------------------------------------------- 3D Skin Renderer */

/* ------------------------------------------------ Catmull-Rom Live FPS Spline */

let fpsCanvasAnimId = null;
const fpsHistory = [680, 710, 740, 725, 780, 830, 810, 860, 890, 875, 910, 940, 920, 895, 930];

function initFpsSplineGraph() {
	const canvas = $('#fps-spline-canvas');
	if (!canvas) return;
	const ctx = canvas.getContext('2d');
	if (!ctx) return;

	// The backing store follows the CSS box and the screen density: the fixed 600 px canvas was
	// stretched by the layout and read as a blurry line.
	const fit = () => {
		const ratio = window.devicePixelRatio || 1;
		const box = canvas.getBoundingClientRect();
		canvas.width = Math.max(1, Math.round(box.width * ratio));
		canvas.height = Math.max(1, Math.round(box.height * ratio));
	};
	fit();
	window.addEventListener('resize', fit);

	function catmullRom(p0, p1, p2, p3, t) {
		const t2 = t * t;
		const t3 = t2 * t;
		return 0.5 * (
			(2 * p1) +
			(-p0 + p2) * t +
			(2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 +
			(-p0 + 3 * p1 - 3 * p2 + p3) * t3
		);
	}

	let tickCounter = 0;
	function renderGraph() {
		tickCounter++;
		if (tickCounter % 30 === 0) {
			const jitter = Math.floor((Math.random() - 0.45) * 40);
			const last = fpsHistory[fpsHistory.length - 1];
			const next = Math.max(550, Math.min(1050, last + jitter));
			fpsHistory.push(next);
			if (fpsHistory.length > 24) fpsHistory.shift();

			const badge = $('#fps-spline-status');
			if (badge) badge.textContent = `${next} FPS · sample`;
		}

		ctx.clearRect(0, 0, canvas.width, canvas.height);
		const w = canvas.width;
		const h = canvas.height;

		// Background grid lines
		ctx.strokeStyle = 'rgba(255, 255, 255, 0.04)';
		ctx.lineWidth = 1;
		for (let y = 20; y < h; y += 25) {
			ctx.beginPath();
			ctx.moveTo(0, y);
			ctx.lineTo(w, y);
			ctx.stroke();
		}

		// Calculate point coordinates
		const minVal = 500;
		const maxVal = 1100;
		const step = w / (fpsHistory.length - 1);
		const points = fpsHistory.map((val, i) => ({
			x: i * step,
			y: h - ((val - minVal) / (maxVal - minVal)) * (h - 24) - 12
		}));

		if (points.length < 2) return;

		// Gradient Fill under spline
		const grad = ctx.createLinearGradient(0, 0, 0, h);
		grad.addColorStop(0, 'rgba(231, 233, 236, 0.10)');
		grad.addColorStop(0.6, 'rgba(231, 233, 236, 0.04)');
		grad.addColorStop(1, 'rgba(231, 233, 236, 0)');

		ctx.beginPath();
		ctx.moveTo(points[0].x, points[0].y);

		for (let i = 0; i < points.length - 1; i++) {
			const p0 = points[Math.max(0, i - 1)];
			const p1 = points[i];
			const p2 = points[i + 1];
			const p3 = points[Math.min(points.length - 1, i + 2)];

			for (let t = 0; t < 1; t += 0.1) {
				const x = catmullRom(p0.x, p1.x, p2.x, p3.x, t);
				const y = catmullRom(p0.y, p1.y, p2.y, p3.y, t);
				ctx.lineTo(x, y);
			}
		}

		const lastPoint = points[points.length - 1];
		ctx.lineTo(lastPoint.x, lastPoint.y);

		// Fill area
		ctx.save();
		ctx.lineTo(w, h);
		ctx.lineTo(0, h);
		ctx.closePath();
		ctx.fillStyle = grad;
		ctx.fill();
		ctx.restore();

		// Spline stroke line
		ctx.beginPath();
		ctx.moveTo(points[0].x, points[0].y);
		for (let i = 0; i < points.length - 1; i++) {
			const p0 = points[Math.max(0, i - 1)];
			const p1 = points[i];
			const p2 = points[i + 1];
			const p3 = points[Math.min(points.length - 1, i + 2)];

			for (let t = 0; t <= 1; t += 0.1) {
				const x = catmullRom(p0.x, p1.x, p2.x, p3.x, t);
				const y = catmullRom(p0.y, p1.y, p2.y, p3.y, t);
				ctx.lineTo(x, y);
			}
		}
		ctx.strokeStyle = themeColors().graph;
		ctx.lineWidth = 1.5;
		ctx.stroke();

		fpsCanvasAnimId = requestAnimationFrame(renderGraph);
	}

	if (fpsCanvasAnimId) cancelAnimationFrame(fpsCanvasAnimId);
	fpsCanvasAnimId = requestAnimationFrame(renderGraph);
}

/**
 * Runs an action of the interface.
 *
 * The controls are given back even when the bridge rejects: without the `finally`, one failed
 * answer left the buttons disabled and the action impossible to retry until the app restarted.
 */
async function withBusy(reset, work) {
	try {
		return await work();
	} catch (error) {
		setStatus('Error', 'error');
		toast(error?.message || 'The action could not be completed.', 'error');
		return null;
	} finally {
		reset();
	}
}

/** The colour of an account's avatar: the same name always paints the same hue. */
function avatarHue(name) {
	let hash = 0;
	for (let i = 0; i < name.length; i++) {
		hash = name.charCodeAt(i) + ((hash << 5) - hash);
	}
	return Math.abs(hash) % 360;
}

/** `hsl()` to three channels, so the ink of the initial can be measured like any other colour. */
function hslToRgb(hue, saturation, lightness) {
	const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
	const section = (hue / 60) % 6;
	const second = chroma * (1 - Math.abs((section % 2) - 1));
	const [red, green, blue] =
		section < 1 ? [chroma, second, 0]
		: section < 2 ? [second, chroma, 0]
		: section < 3 ? [0, chroma, second]
		: section < 4 ? [0, second, chroma]
		: section < 5 ? [second, 0, chroma]
		: [chroma, 0, second];
	const floor = lightness - chroma / 2;

	return [red, green, blue].map((channel) => Math.round((channel + floor) * 255));
}

/**
 * Paints the avatar of an account: the hue of its name, and the ink that stays readable on it.
 *
 * The hue alone used to decide the colour with a fixed white label, which measured 1.99:1 on a
 * green avatar: the ink is now chosen on the colour, like every other coloured surface.
 */
function paintAvatar(element, name) {
	const hue = avatarHue(name);
	// Quiet hues: two accounts stay distinguishable without six saturated blocks in a table.
	element.style.background = `hsl(${hue}, 20%, 24%)`;
	element.style.color = contrastInk(hslToRgb(hue, 0.20, 0.24));
}

function formatBytes(bytes) {
	if (!bytes) return '0 B';
	const units = ['B', 'KB', 'MB', 'GB'];
	let index = 0;
	let value = bytes;
	while (value >= 1024 && index < units.length - 1) {
		value /= 1024;
		index++;
	}
	return `${value.toFixed(value >= 10 || index === 0 ? 0 : 1)} ${units[index]}`;
}

/* --------------------------------------------------------------- rendering */

const ICON_USER = '<svg viewBox="0 0 24 24"><circle cx="12" cy="8" r="4" /><path d="M4 21c0-4.4 3.6-7 8-7s8 2.6 8 7" /></svg>';
const ICON_STACK = '<svg viewBox="0 0 24 24"><path d="M12 3l9 5-9 5-9-5z" /><path d="M3 13l9 5 9-5" /></svg>';
const ICON_MOD = '<svg viewBox="0 0 24 24"><path d="M10 4h4v2.6a2.2 2.2 0 002.6 2.4 2.2 2.2 0 010 4.4A2.2 2.2 0 0014 16v4h-4v-2.6a2.2 2.2 0 00-2.6-2.4 2.2 2.2 0 010-4.4A2.2 2.2 0 0010 8z" /></svg>';
const ICON_LOG = '<svg viewBox="0 0 24 24"><path d="M6 3h9l4 4v14H6z" /><path d="M9 8h3M9 12h7M9 16h7" /></svg>';
const ICON_SERVER = '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3c2.8 2.6 4 5.7 4 9s-1.2 6.4-4 9c-2.8-2.6-4-5.7-4-9s1.2-6.4 4-9z" /></svg>';
const ICON_GAUGE = '<svg viewBox="0 0 24 24"><path d="M4 17a8 8 0 1116 0" /><path d="M12 17l4-5" /></svg>';

/** A list without entries says what it is waiting for instead of leaving a blank card. */
function emptyState(icon, title, hint, actionLabel) {
	const wrapper = document.createElement('div');
	wrapper.className = 'empty';
	wrapper.innerHTML = `
		<span class="empty-icon">${icon}</span>
		<p class="empty-title"></p>
		<p class="empty-hint"></p>
		${actionLabel ? '<button class="btn btn--primary" data-empty-action></button>' : ''}
	`;
	wrapper.querySelector('.empty-title').textContent = title;
	wrapper.querySelector('.empty-hint').textContent = hint;
	if (actionLabel) wrapper.querySelector('[data-empty-action]').textContent = actionLabel;
	return wrapper;
}

function renderAccounts() {
	const list = $('#play-accounts');
	list.innerHTML = '';

	if (state.accounts.accounts.length === 0) {
		list.appendChild(emptyState(
			ICON_USER,
			'No account yet',
			'Add an offline account to play right away, it only needs a player name.'
		));
	} else {
		for (const account of state.accounts.accounts) {
			const row = document.createElement('div');
			row.className = `account-row${account.id === state.accounts.selected ? ' is-selected' : ''}`;

			const avatar = document.createElement('div');
			avatar.className = 'avatar';
			paintAvatar(avatar, account.username);
			avatar.textContent = account.username.charAt(0);

			const meta = document.createElement('div');
			meta.className = 'account-meta';
			const name = document.createElement('div');
			name.className = 'account-name';
			name.textContent = account.username;
			const uuid = document.createElement('div');
			uuid.className = 'account-uuid';
			// The identifier is 32 hexadecimal characters: eight are enough to tell two accounts
			// apart, the full value stays in the tooltip and in the configuration.
			uuid.textContent = account.uuid.slice(0, 8);
			uuid.title = account.uuid;
			meta.append(name, uuid);

			const tag = document.createElement('span');
			tag.className = `tag tag--${account.type}`;
			tag.textContent = account.type === 'offline' ? 'Offline' : 'Microsoft';

			row.append(avatar, meta, tag);
			row.addEventListener('click', async () => {
				state.accounts = await api.accounts.select(account.id);
				renderAccounts();
				renderAccountTable();
				await loadProfiles();
			});
			list.appendChild(row);
		}
	}

	const selected = state.accounts.accounts.find((a) => a.id === state.accounts.selected);
	$('#session-badge').textContent = selected
		? (selected.type === 'offline' ? 'offline session' : 'microsoft session')
		: 'no account selected';

	renderDock();
}

function renderAccountTable() {
	const table = $('#account-table');
	table.innerHTML = '';

	if (state.accounts.accounts.length === 0) {
		const empty = emptyState(
			ICON_USER,
			'No accounts yet',
			'Offline accounts are created locally. Microsoft accounts use the device code sign in.',
			'Add an offline account'
		);
		empty.querySelector('[data-empty-action]').addEventListener('click', offlineAccountModal);
		table.appendChild(empty);
		return;
	}

	for (const account of state.accounts.accounts) {
		const row = document.createElement('div');
		row.className = `account-row${account.id === state.accounts.selected ? ' is-selected' : ''}`;

		const avatar = document.createElement('div');
		avatar.className = 'avatar';
		paintAvatar(avatar, account.username);
		avatar.textContent = account.username.charAt(0);

		const meta = document.createElement('div');
		meta.className = 'account-meta';
		const name = document.createElement('div');
		name.className = 'account-name';
		name.textContent = account.username;
		const uuid = document.createElement('div');
		uuid.className = 'account-uuid';
		uuid.textContent = account.uuid.slice(0, 8);
		uuid.title = account.uuid;
		meta.append(name, uuid);

		const tag = document.createElement('span');
		tag.className = `tag tag--${account.type}`;
		tag.textContent = account.type === 'offline' ? 'Offline' : 'Microsoft';

		const select = document.createElement('button');
		select.className = 'btn btn--ghost';
		select.textContent = account.id === state.accounts.selected ? 'Selected' : 'Select';
		select.addEventListener('click', async () => {
			state.accounts = await api.accounts.select(account.id);
			renderAccounts();
			renderAccountTable();
			await loadProfiles();
		});

		const remove = document.createElement('button');
		remove.className = 'icon-btn';
		remove.innerHTML = '<svg viewBox="0 0 24 24"><path d="M4 7h16M9 7V5h6v2M6 7l1 13h10l1-13" /></svg>';
		remove.title = 'Remove';
		remove.addEventListener('click', async () => {
			state.accounts = await api.accounts.remove(account.id);
			renderAccounts();
			renderAccountTable();
			toast(`Removed ${account.username}`, 'success');
		});

		row.append(avatar, meta, tag, select, remove);
		table.appendChild(row);
	}
}

function renderVersions() {
	const table = $('#version-table');
	table.innerHTML = '';

	if (state.versions.length === 0) {
		table.appendChild(emptyState(
			ICON_STACK,
			'Reading the version list…',
			'The versions are published by Mojang, the list fills itself as soon as the answer arrives.'
		));
		return;
	}

	for (const version of state.versions) {
		const row = document.createElement('div');
		row.className = 'version-row';

		const left = document.createElement('div');
		const id = document.createElement('div');
		id.className = 'version-id';
		id.textContent = version.id;
		const date = document.createElement('div');
		date.className = 'version-date';
		date.textContent = new Date(version.releaseTime).toLocaleDateString();
		left.append(id, date);

		const action = document.createElement('button');
		action.className = 'btn btn--ghost';
		action.textContent = 'Use';
		action.addEventListener('click', async () => {
			await updateProfile({ version: version.id });
			toast(`Selected Minecraft ${version.id}`, 'success');
			loadLoaders(version.id);
		});

		row.append(left, action);
		table.appendChild(row);
	}
}

function renderVersionSelect() {
	const select = $('#select-version');
	select.innerHTML = '';

	// Without a network the list of Mojang cannot be read, the version of the profile still has to
	// be selectable instead of leaving an empty dropdown behind.
	const wanted = currentProfile().gameVersion;
	const versions = state.versions.length > 0 ? state.versions : [{ id: wanted }];

	for (const version of versions) {
		const option = document.createElement('option');
		option.value = version.id;
		option.textContent = version.id;
		if (version.id === wanted) option.selected = true;
		select.appendChild(option);
	}
}

function renderLoaders() {
	const select = $('#select-loader');
	const wanted = currentProfile().loaderVersion;
	select.innerHTML = '';

	if (state.loaders.length === 0) {
		const option = document.createElement('option');
		option.value = wanted || '';
		option.textContent = wanted || 'default';
		select.appendChild(option);
		return;
	}
	for (const loader of state.loaders) {
		const option = document.createElement('option');
		option.value = loader;
		option.textContent = loader;
		if (loader === wanted) option.selected = true;
		select.appendChild(option);
	}
}

/** The number of lines that are kept, a long session must not grow without a limit. */
const MAX_CONSOLE_LINES = 2000;

/** The output of the game is shown twice: next to the launch button and on the page of the logs. */
const consoles = () => ['#console', '#session-log'].map((selector) => $(selector));

function log(line) {
	for (const element of consoles()) {
		element.appendChild(document.createTextNode(`${line}\n`));

		while (element.childNodes.length > MAX_CONSOLE_LINES) {
			element.removeChild(element.firstChild);
		}

		element.scrollTop = element.scrollHeight;
	}

	state.sessionLines += 1;
	$('#session-log-badge').textContent =
		state.sessionLines === 1 ? '1 line' : `${state.sessionLines} lines`;
	$('#session-log-badge').classList.add('is-live');
}

/** A new game starts a new session, the previous output is dropped. */
function clearSession() {
	for (const element of consoles()) {
		element.textContent = '';
	}

	state.sessionLines = 0;
	$('#session-log-badge').textContent = 'empty';
	$('#session-log-badge').classList.remove('is-live');
}

/* -------------------------------------------------------------------- mods */

function renderMods() {
	const table = $('#mods-table');
	table.innerHTML = '';

	const enabled = state.mods.filter((entry) => entry.enabled).length;
	$('#mods-count').textContent = state.mods.length === 0
		? 'no mod'
		: `${enabled}/${state.mods.length} on`;

	if (state.mods.length === 0) {
		const empty = emptyState(
			ICON_MOD,
			'No mod in the folder',
			'Drop a jar in the mods folder, or run “Install / repair” to download the Aether client.',
			'Open the folder'
		);
		empty.querySelector('[data-empty-action]').addEventListener('click', openModsFolder);
		table.appendChild(empty);
		return;
	}

	for (const entry of state.mods) {
		const row = document.createElement('div');
		row.className = `mod-row${entry.enabled ? '' : ' is-disabled'}`;
		row.title = entry.path;

		const icon = document.createElement('span');
		icon.className = 'mod-icon';
		icon.innerHTML = ICON_MOD;

		const meta = document.createElement('div');
		meta.className = 'mod-meta';
		const name = document.createElement('div');
		name.className = 'mod-name';
		name.textContent = entry.modId;
		const sub = document.createElement('div');
		sub.className = 'mod-sub';
		sub.textContent = `${formatBytes(entry.size)} · ${entry.enabled ? 'enabled' : 'disabled'}`;
		meta.append(name, sub);

		const toggle = document.createElement('button');
		toggle.className = 'switch';
		toggle.setAttribute('role', 'switch');
		toggle.setAttribute('aria-checked', String(entry.enabled));
		toggle.setAttribute('aria-label', `${entry.enabled ? 'Disable' : 'Enable'} ${entry.modId}`);
		toggle.addEventListener('click', async () => {
			toggle.disabled = true;
			const result = await api.mods.toggle(entry.name, !entry.enabled);
			toggle.disabled = false;

			if (!result.ok) {
				toast(result.error || 'The mod could not be changed.', 'error');
				return;
			}

			state.mods = result.mods;
			renderMods();
			toast(`${entry.modId} ${entry.enabled ? 'disabled' : 'enabled'}`, 'success');
		});

		row.append(icon, meta, toggle);
		table.appendChild(row);
	}
}

async function loadMods() {
	const result = await api.mods.list();
	if (!result.ok) {
		toast(result.error || 'The mods could not be read.', 'error');
		return;
	}

	state.mods = result.mods || [];
	$('#mods-path').textContent = result.folder || '';
	renderMods();
}

async function openModsFolder() {
	const result = await api.mods.openFolder();
	if (!result.ok) toast(result.error || 'The folder could not be opened.', 'error');
}

/* ----------------------------------------------------------------- servers */

/** Builds a small button that lives in the actions of a row. */
function rowButton(label, variant, onClick) {
	const button = document.createElement('button');
	button.className = `btn ${variant} btn--small`;
	button.textContent = label;
	button.addEventListener('click', onClick);
	return button;
}

/** A badge that says whether something is installed or running. */
function rowBadge(label, live) {
	const badge = document.createElement('span');
	badge.className = `badge${live ? ' is-live' : ''}`;
	badge.textContent = label;
	return badge;
}

/** The one line summary of a ping, empty when the server was never pinged. */
function pingSummary(ping) {
	if (!ping) return '';
	if (!ping.online) return ping.error ? `offline · ${ping.error}` : 'offline';

	const parts = [];
	if (ping.playersMax > 0) parts.push(`${ping.playersOnline}/${ping.playersMax}`);
	parts.push(`${ping.latencyMs} ms`);
	if (ping.versionName) parts.push(ping.versionName);
	return parts.join(' · ');
}

function renderServers() {
	const table = $('#server-list');
	table.innerHTML = '';

	const list = state.servers.servers || [];
	$('#server-count').textContent = list.length === 1 ? '1 server' : `${list.length} servers`;

	if (list.length === 0) {
		table.appendChild(emptyState(
			ICON_SERVER,
			'No server saved yet',
			'Add an address above, or host a local server that accepts any name.'
		));
		return;
	}

	for (const entry of list) {
		const ping = state.servers.pings[entry.address];
		const isOnline = ping ? ping.online : false;

		const card = document.createElement('div');
		card.className = 'server-card-discord mod-row'; // retain mod-row class for selector tests

		// Header row
		const head = document.createElement('div');
		head.className = 'server-card-discord-header';

		const titleGroup = document.createElement('div');
		titleGroup.className = 'server-card-discord-title-group';

		const dot = document.createElement('span');
		dot.className = `server-status-dot-pulse${isOnline ? '' : ' is-offline'}`;

		const name = document.createElement('div');
		name.className = 'server-card-discord-name mod-name';
		name.textContent = entry.name;

		const addr = document.createElement('div');
		addr.className = 'server-card-discord-address mod-sub';
		addr.textContent = entry.address;

		titleGroup.append(dot, name, addr);

		// Ping / Players badges
		const badges = document.createElement('div');
		badges.className = 'actions actions--tight';

		if (ping && ping.online) {
			const latBadge = document.createElement('span');
			latBadge.className = 'badge is-live';
			latBadge.textContent = `${ping.latencyMs} ms`;
			badges.appendChild(latBadge);

			if (ping.playersMax > 0) {
				const plBadge = document.createElement('span');
				plBadge.className = 'badge';
				plBadge.textContent = `${ping.playersOnline}/${ping.playersMax}`;
				badges.appendChild(plBadge);
			}
		} else if (ping && !ping.online) {
			const offBadge = document.createElement('span');
			offBadge.className = 'badge';
			offBadge.textContent = 'Offline';
			badges.appendChild(offBadge);
		}

		head.append(titleGroup, badges);

		// MOTD View (parsed minecraft color codes)
		const motdView = document.createElement('div');
		motdView.className = 'motd-container';
		motdView.innerHTML = parseMinecraftMotd(ping?.motd || '');

		// Actions footer
		const actions = document.createElement('div');
		actions.className = 'row-actions';

		const join = rowButton('Join', 'btn--ghost', () => launch(entry.address));
		const test = rowButton('Test', 'btn--ghost', async () => {
			test.disabled = true;
			await testAddress(entry.address);
			test.disabled = false;
		});
		const remove = rowButton('Remove', 'btn--danger', async () => {
			remove.disabled = true;
			const result = await api.servers.remove(entry.address);
			if (!result.ok) {
				remove.disabled = false;
				toast(result.error || 'The server could not be removed.', 'error');
				return;
			}

			state.servers.servers = result.servers || [];
			delete state.servers.pings[entry.address];
			renderServers();
			toast(`${entry.name} removed.`, 'success');
		});

		actions.append(join, test, remove);

		card.append(head, motdView, actions);
		table.appendChild(card);
	}
}

async function loadServers() {
	const result = await api.servers.list();
	if (!result.ok) {
		toast(result.error || 'The server list could not be read.', 'error');
		return;
	}

	state.servers.servers = result.servers || [];
	renderServers();
}

async function pingServer(address) {
	const result = await api.servers.ping(address);
	if (!result.ok) return null;

	const saved = state.servers.servers.find(
		(entry) => entry.address.toLowerCase() === address.toLowerCase()
	);
	state.servers.pings[saved ? saved.address : address] = result.ping;
	if (saved) renderServers();
	return result.ping;
}

/** Pings an address and probes its login state, then tells the player the verdict. */
async function testAddress(address) {
	const target = (address || $('#server-address').value || '').trim();
	if (!target) {
		toast('Enter an address to test.', 'error');
		return;
	}

	const verdict = $('#server-verdict');
	verdict.textContent = `Testing ${target}…`;

	const [ping, probe] = await Promise.all([
		api.servers.ping(target),
		api.servers.probe(target)
	]);

	const saved = state.servers.servers.find(
		(entry) => entry.address.toLowerCase() === target.toLowerCase()
	);
	if (ping.ok && saved) {
		state.servers.pings[saved.address] = ping.ping;
		renderServers();
	}

	const parts = [];
	if (probe.ok) {
		const result = probe.probe;
		if (probe.verdict === 'offline') {
			parts.push(`Offline accounts accepted (the server answered with ${result.packet}).`);
		} else if (probe.verdict === 'premium') {
			parts.push(`Premium only: ${result.detail}.`);
		} else if (probe.verdict === 'refused') {
			parts.push(`Refused: ${result.detail}.`);
		} else {
			parts.push(`Unreachable: ${result.detail}.`);
		}
	} else {
		parts.push(probe.error || 'The login probe failed.');
	}

	if (ping.ok) {
		const info = ping.ping;
		parts.push(info.online
			? `${info.playersOnline}/${info.playersMax} players · ${info.latencyMs} ms · ${info.versionName}`
			: `offline: ${info.error}`);
	}

	verdict.textContent = parts.join(' ');
	const accepted = probe.ok && probe.verdict === 'offline';
	// The verdict stays on the page: a toast would cover the rows while the player reads it.
	if (accepted) toast(`${target} accepts an offline account.`, 'success');
}

async function addServer() {
	const address = $('#server-address').value.trim();
	if (!address) {
		toast('Enter the address of a server.', 'error');
		return;
	}

	const name = $('#server-name').value.trim() || address;
	const button = $('#btn-server-add');
	button.disabled = true;

	const result = await withBusy(
		() => {
			button.disabled = false;
		},
		() => api.servers.add(name, address)
	);

	if (!result) return;
	if (!result.ok) {
		toast(result.error || 'The server could not be added.', 'error');
		return;
	}

	state.servers.servers = result.servers || [];
	$('#server-name').value = '';
	$('#server-address').value = '';
	renderServers();
	toast(`${name} added.`, 'success');
	pingServer(address);
}

/* --------------------------------------------------------------- local servers */

function renderHosts() {
	const table = $('#host-list');
	table.innerHTML = '';

	const list = state.hosts || [];
	$('#host-count').textContent = list.length === 1 ? '1 server' : `${list.length} servers`;

	if (list.length === 0) {
		table.appendChild(emptyState(
			ICON_SERVER,
			'No local server',
			'Create one above: the launcher installs Fabric once, then the world starts with online-mode=false.'
		));
		return;
	}

	for (const server of list) {
		const address = `127.0.0.1:${server.port}`;
		const row = document.createElement('div');
		row.className = 'mod-row';
		row.title = server.directory;

		const icon = document.createElement('span');
		icon.className = 'mod-icon';
		icon.innerHTML = ICON_SERVER;

		const meta = document.createElement('div');
		meta.className = 'mod-meta';
		const name = document.createElement('div');
		name.className = 'mod-name';
		name.textContent = server.name;
		const sub = document.createElement('div');
		sub.className = 'mod-sub';
		sub.textContent = `${address} · ${server.onlineMode ? 'online mode' : 'offline mode'} · ${server.gameVersion}`;
		meta.append(name, sub);

		const actions = document.createElement('div');
		actions.className = 'row-actions';
		actions.appendChild(rowBadge(server.running ? 'running' : 'stopped', server.running));

		if (server.running) {
			actions.appendChild(rowButton('Join', 'btn--primary', () => launch(address)));
			actions.appendChild(rowButton('Stop', 'btn--danger', () => stopHost(server.name)));
		} else {
			actions.appendChild(rowButton('Start', 'btn--primary', () => startHost(server.name)));
		}

		row.append(icon, meta, actions);
		table.appendChild(row);
	}
}

async function loadHosts() {
	const result = await api.hosts.list();
	if (!result.ok) {
		toast(result.error || 'The local servers could not be read.', 'error');
		return;
	}

	state.hosts = result.servers || [];
	renderHosts();
}

async function createHost() {
	const name = $('#host-name').value.trim();
	if (!name) {
		toast('Give the world a name.', 'error');
		return;
	}

	const button = $('#btn-host-create');
	button.disabled = true;

	const result = await withBusy(
		() => {
			button.disabled = false;
		},
		() => api.hosts.create({
			name,
			port: Number($('#host-port').value) || 25565,
			maxPlayers: Number($('#host-players').value) || 20
		})
	);

	if (!result) return;
	if (!result.ok) {
		toast(result.error || 'The server could not be created.', 'error');
		return;
	}

	state.hosts = result.servers || [];
	$('#host-name').value = '';
	renderHosts();
	toast(`${name} created. Press Start to launch it.`, 'success');
}

/** Pings until the server answers, a first start compiles its world for a few seconds. */
async function waitForServer(address, attempts = 90) {
	for (let index = 0; index < attempts; index++) {
		const result = await api.servers.ping(address);
		if (result.ok && result.ping?.online) {
			state.servers.pings[address] = result.ping;
			return true;
		}

		await new Promise((resolve) => setTimeout(resolve, 2000));
	}

	return false;
}

async function startHost(name) {
	setStatus('Starting server…', 'busy');
	const result = await withBusy(() => setStatus('Ready'), () => api.hosts.start(name));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || 'The server could not be started.', 'error');
		return;
	}

	await loadHosts();
	toast(`${name} is starting…`, 'success');

	const address = `127.0.0.1:${result.server.port}`;
	const ready = await waitForServer(address);

	if (ready) {
		renderHosts();
		toast(`${name} is ready on ${address}.`, 'success');
	} else {
		toast(`${name} is still starting, watch console.log in its folder.`, 'error');
	}
}

async function stopHost(name) {
	setStatus('Stopping server…', 'busy');
	const result = await withBusy(() => setStatus('Ready'), () => api.hosts.stop(name));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || 'The server could not be stopped.', 'error');
		return;
	}

	state.hosts = result.servers || state.hosts;
	renderHosts();
	toast(`${name} stopped and the world was saved.`, 'success');
}

/* ------------------------------------------------------------- performance */

/** The version number of a mod without the noise of its Minecraft suffix. */
function displayModVersion(version) {
	if (!version) return '';

	return version
		.replace(/^fabric-/, '')
		.replace(/^mc[0-9.]+-/, '')
		.replace(/\+minecraft-[0-9.]+(-fabric)?/, '')
		.replace(/\+mc[0-9.]+/, '')
		.replace(/-fabric$/, '')
		.replace(/-1\.21\.11$/, '');
}

function renderPerformance() {
	const table = $('#perf-mods');
	table.innerHTML = '';

	const mods = state.performance.mods || [];
	const installed = mods.filter((mod) => mod.installed).length;
	$('#perf-count').textContent = `${installed}/${mods.length}`;
	$('#perf-path').textContent = state.performance.state?.modsFolder || '';

	if (mods.length === 0) {
		table.appendChild(emptyState(
			ICON_GAUGE,
			'The pack is not readable',
			'Refresh the list, the core reads the mods folder of the game.'
		));
		return;
	}

	for (const mod of mods) {
		const row = document.createElement('div');
		row.className = `mod-row${mod.installed ? '' : ' is-disabled'}`;

		const icon = document.createElement('span');
		icon.className = 'mod-icon';
		icon.innerHTML = ICON_MOD;

		const meta = document.createElement('div');
		meta.className = 'mod-meta';
		const name = document.createElement('div');
		name.className = 'mod-name';
		name.textContent = mod.name;
		const sub = document.createElement('div');
		sub.className = 'mod-sub';
		const version = displayModVersion(mod.version);
		sub.textContent = mod.installed && version ? `${mod.description} · ${version}` : mod.description;
		meta.append(name, sub);

		row.append(icon, meta, rowBadge(mod.installed ? 'installed' : 'missing', mod.installed));
		table.appendChild(row);
	}
}

function renderPerformanceState() {
	const settings = state.performance.state || {};

	if (settings.jvmPreset) {
		selectOption($('#select-jvm'), settings.jvmPreset);
		$('#jvm-current').textContent = `Current preset: ${settings.jvmPreset}. It applies on the next launch.`;

		// Highlight active preset card
		$$('.preset-card').forEach((card) => {
			card.classList.toggle('is-active', card.dataset.preset === settings.jvmPreset);
		});
	}

	const preset = settings.optionsPreset;
	if (preset && preset !== 'vanilla') {
		$('#options-current').textContent = preset === 'restore'
			? 'The original options.txt is in place.'
			: `Last applied preset: ${preset}.`;
	} else {
		$('#options-current').textContent = 'The original options.txt is in place.';
	}
}

async function loadPerformance() {
	const result = await api.performance.list();
	if (!result.ok) {
		toast(result.error || 'The optimisation pack could not be read.', 'error');
		return;
	}

	state.performance.mods = result.mods || [];
	state.performance.state = result.state || {};
	renderPerformance();
	renderPerformanceState();
}

async function installPack() {
	if (state.performance.installing) return;

	state.performance.installing = true;
	const button = $('#btn-perf-install');
	button.disabled = true;
	$('#perf-path').textContent = 'Resolving the pack on Modrinth…';

	const result = await withBusy(
		() => {
			state.performance.installing = false;
			button.disabled = false;
		},
		() => api.performance.install([])
	);

	if (!result) return;
	if (!result.mods) {
		toast(result.error || 'The pack could not be installed.', 'error');
		return;
	}

	state.performance.mods = result.mods;
	state.performance.state = result.state || state.performance.state;
	renderPerformance();
	renderPerformanceState();

	const failed = (result.report?.failed || []).length;
	if (failed > 0) {
		toast(`${failed} mod(s) could not be installed.`, 'error');
	} else {
		toast('The optimisation pack is installed. Restart the game.', 'success');
	}
}

async function tuneOptions(preset) {
	const result = await withBusy(() => {}, () => api.performance.tune(preset));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || 'The options could not be written.', 'error');
		return;
	}

	state.performance.state = result.state || state.performance.state;
	renderPerformanceState();

	if (preset === 'restore') {
		toast('The original options are back.', 'success');
		return;
	}

	const changes = result.report?.changes?.length || 0;
	toast(`${preset === 'max' ? 'Maximum FPS' : 'Balanced'}: ${changes} option(s) changed.`, 'success');
}

async function applyJvm() {
	const preset = $('#select-jvm').value;
	const result = await withBusy(() => {}, () => api.performance.jvm(preset));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || 'The JVM preset could not be saved.', 'error');
		return;
	}

	state.performance.state = result.state || state.performance.state;
	renderPerformanceState();
	toast(`JVM preset set to ${preset}, it applies on the next launch.`, 'success');
}

/* -------------------------------------------------------------------- logs */

async function loadLogs() {
	const result = await api.logs.tail(200);

	if (result.ok) {
		renderSessions(result.sessions || []);
	} else {
		renderSessionsError(result.error);
	}

	renderInstallLog(result);

	// The core removes the oldest sessions, the interface says how many are kept.
	$('#sessions-note').textContent = result.ok
		? `The last ${result.keep} sessions are kept, older files are removed automatically.`
		: '';
}

/** A list of sessions that could not be read says why instead of looking empty. */
function renderSessionsError(error) {
	const table = $('#sessions-table');
	table.innerHTML = '';
	$('#sessions-count').textContent = 'unreadable';
	table.appendChild(emptyState(
		ICON_LOG,
		'The sessions could not be read',
		error || 'The core did not answer.'
	));
}

function renderSessions(sessions) {
	const table = $('#sessions-table');
	table.innerHTML = '';

	$('#sessions-count').textContent = sessions.length === 0
		? 'no session'
		: sessions.length === 1 ? '1 session' : `${sessions.length} sessions`;

	if (sessions.length === 0) {
		table.appendChild(emptyState(
			ICON_LOG,
			'No session yet',
			'Every launch writes the output of the game to aether/logs/session-<date>.log.'
		));
		return;
	}

	for (const entry of sessions) {
		const row = document.createElement('div');
		row.className = 'session-row';
		row.title = entry.path;

		const icon = document.createElement('span');
		icon.className = 'mod-icon';
		icon.innerHTML = ICON_LOG;

		const meta = document.createElement('div');
		meta.className = 'mod-meta';
		const name = document.createElement('div');
		name.className = 'mod-name';
		name.textContent = entry.started || entry.name;
		const sub = document.createElement('div');
		sub.className = 'mod-sub';
		sub.append(`${formatBytes(entry.size)} · `);
		const fileName = document.createElement('span');
		fileName.className = 'log-name';
		fileName.textContent = entry.name;
		sub.append(fileName);
		meta.append(name, sub);

		const view = document.createElement('button');
		view.className = 'btn btn--ghost';
		view.textContent = 'View';
		view.addEventListener('click', () => openSession(entry));

		row.append(icon, meta, view);
		table.appendChild(row);
	}
}

/** The session that is being opened, a slow answer must not replace a newer one. */
let sessionRequest = 0;

/** The last lines of one run of the game, read back from its file. */
async function openSession(entry) {
	const view = $('#session-view');
	const request = ++sessionRequest;
	const result = await api.logs.read(entry.name, 400);

	if (request !== sessionRequest) return;
	view.innerHTML = '';

	if (!result.ok) {
		view.appendChild(emptyState(
			ICON_LOG,
			'The session could not be read',
			result.error || 'The core did not answer.'
		));
		return;
	}

	const tail = result.session;
	const label = document.createElement('p');
	label.className = 'log-path';
	label.textContent = `${tail.total} lines · ${tail.path}`;

	const pre = document.createElement('pre');
	pre.className = 'console console--tall';
	pre.id = 'session-file';
	pre.textContent = tail.lines.join('\n');

	view.append(label, pre);
	// The log opens under the list, the page follows it so the click has a visible answer.
	pre.scrollIntoView({ block: 'nearest' });
}

function renderInstallLog(result) {
	const body = $('#install-log-body');
	body.innerHTML = '';

	if (!result.ok) {
		body.appendChild(emptyState(
			ICON_LOG,
			'The log could not be read',
			result.error || 'The core did not answer.'
		));
		return;
	}

	const tail = result.install;
	$('#install-log-path').textContent = tail.exists
		? `${tail.total} lines · ${tail.path}`
		: tail.path;

	if (!tail.exists || tail.lines.length === 0) {
		body.appendChild(emptyState(
			ICON_LOG,
			'No installation logged yet',
			'Every download writes a line here. Run “Install / repair” to fill it.'
		));
		return;
	}

	const pre = document.createElement('pre');
	pre.className = 'console console--tall';
	pre.id = 'install-log';
	pre.textContent = tail.lines.join('\n');
	body.appendChild(pre);
}

/* ------------------------------------------------------------------ modals */

/** The element that had the focus when the dialog opened, the focus goes back to it. */
let modalReturnFocus = null;

function openModal(title, bodyHtml, onConfirm, confirmLabel = 'Confirm') {
	modalReturnFocus = document.activeElement;
	$('#modal-title').textContent = title;
	$('#modal-body').innerHTML = bodyHtml;
	$('#modal-confirm').textContent = confirmLabel;
	$('#modal').hidden = false;

	// The dialog takes the focus, on its first field when it has one.
	const first = $('#modal-body').querySelector('input, select, textarea');
	(first || $('.modal')).focus();

	const confirm = $('#modal-confirm');
	confirm.onclick = async () => {
		confirm.onclick = null;
		closeModal();
		if (typeof onConfirm === 'function') await onConfirm();
	};
}

function closeModal() {
	if ($('#modal').hidden) return;

	$('#modal-confirm').onclick = null;
	$('#modal').hidden = true;

	// The keyboard is not left nowhere: Escape and Confirm both give the focus back.
	const target = modalReturnFocus;
	modalReturnFocus = null;
	if (target && typeof target.focus === 'function' && document.contains(target)) target.focus();
}

function offlineAccountModal() {
	openModal('Add an offline account', `
		<p class="muted">An offline account only needs a name. The identifier is generated exactly
		like Minecraft does it, so the player keeps the same identity on every offline server.</p>
		<label class="field">
			<span class="field-label">Player name</span>
			<input type="text" id="offline-name" class="input" maxlength="16" placeholder="Notch" />
		</label>
		<p class="muted" id="offline-preview">3 to 16 characters, only letters, digits and underscores.</p>
	`, async () => {
		const username = $('#offline-name').value.trim();
		const result = await api.accounts.addOffline(username);
		if (!result.ok) {
			toast(result.error, 'error');
			return;
		}
		state.accounts = await api.accounts.get();
		renderAccounts();
		renderAccountTable();
		toast(`Added ${result.account.username}`, 'success');
	}, 'Create');

	const input = $('#offline-name');
	const preview = $('#offline-preview');
	input.focus();

	// Every keystroke restarts the wait: Mojang is asked once the name has settled, not sixteen
	// times. The answer of an older name is dropped when a newer one is already on its way.
	let waiting = 0;
	let asked = 0;

	const describe = (value, valid) => {
		if (!valid) {
			preview.textContent = '3 to 16 characters, only letters, digits and underscores.';
			preview.style.color = 'var(--text-muted)';
			return;
		}

		preview.textContent = `Ready to play as ${value}.`;
		preview.style.color = 'var(--color-success-text)';

		const ticket = ++asked;
		api.accounts.checkName(value).then((status) => {
			if (ticket !== asked || !status || !status.ok || !status.premium) return;

			preview.textContent =
				`${status.name} belongs to a Mojang account: cracked servers such as JartexNetwork ` +
				`refuse that name ("Unauthorized: You cannot join the server"). Pick a name nobody ` +
				`owns, for example ${value}9281.`;
			preview.style.color = 'var(--color-warning-text)';
		});
	};

	input.addEventListener('input', () => {
		const value = input.value.trim();
		const valid = /^[A-Za-z0-9_]{3,16}$/.test(value);

		clearTimeout(waiting);
		if (!valid) {
			describe(value, false);
			return;
		}

		// The name is valid immediately, the check itself waits for a pause.
		preview.textContent = `Ready to play as ${value}.`;
		preview.style.color = 'var(--color-success-text)';
		waiting = setTimeout(() => describe(value, true), 450);
	});
}

async function microsoftAccountModal() {
	// The core prints the code first and finishes the sign in in the background.
	const pending = api.accounts.microsoft.signIn();

	openModal('Sign in with Microsoft', `
		<p class="muted">Open the following page and enter the code to sign in with your
		Minecraft account.</p>
		<div class="code-box">
			<div class="code-label">Your code</div>
			<div class="code-value" id="device-code">…</div>
		</div>
		<p class="muted" style="margin-top:10px"><a id="verify-link" target="_blank"
			rel="noreferrer">Waiting for the code…</a></p>
	`, async () => {
		setStatus('Signing in…', 'busy');
		const result = await pending;
		setStatus('Ready');
		if (!result.ok) {
			toast(result.error || 'The sign in failed.', 'error');
			return;
		}
		state.accounts = await api.accounts.get();
		renderAccounts();
		renderAccountTable();
		toast(`Signed in as ${result.account.username}`, 'success');
	}, 'I have signed in');
}

/* ----------------------------------------------------------------- actions */

/** The controls of the installation are given back, whatever the outcome. */
function releaseInstall(progress, frozen, card) {
	progress.hidden = true;
	progress.querySelector('.progress-fill').style.width = '0%';
	$('#btn-install').disabled = false;
	$('#btn-cancel-install').hidden = true;
	$('#btn-cancel-install').disabled = false;
	frozen.forEach((control) => {
		control.disabled = false;
	});
	if (card) card.classList.remove('is-busy');
	state.installing = false;
}

async function install() {
	if (state.installing) return;
	state.installing = true;
	setStatus('Installing…', 'busy');

	// The profile is frozen while it downloads, the values of the request cannot change under it.
	const progress = $('#install-progress');
	const frozen = [$('#select-version'), $('#select-loader')];
	const card = progress.closest('.card');

	$('#btn-install').disabled = true;
	$('#btn-cancel-install').hidden = false;
	frozen.forEach((control) => {
		control.disabled = true;
	});
	if (card) card.classList.add('is-busy');
	progress.hidden = false;

	const result = await withBusy(
		() => releaseInstall(progress, frozen, card),
		() =>
			api.install({
				gameVersion: $('#select-version').value,
				loaderVersion: $('#select-loader').value
			})
	);

	// The failure was reported, the interface is usable again.
	if (!result) return;

	if (result.cancelled) {
		setStatus('Ready');
		toast('The installation was cancelled.', 'success');
		return;
	}

	if (!result.ok) {
		setStatus('Error', 'error');
		toast(result.error, 'error');
		loadLogs();
		await showInstallReport(result.error);
		return;
	}
	setStatus('Ready');
	toast('The game is ready to play.', 'success');
	loadLogs();
}

/**
 * What a player can send when a session ends badly, whether the game or the core died.
 *
 * The exit code alone says nothing useful: the version, the loader, the runtime and the last lines
 * of the session are what makes a crash diagnosable. The text is built as one array, the optional
 * line is dropped when there is no code to show.
 */
async function showCrashReport({ title, problem, exitCode }) {
	const logs = await api.logs.tail(20);
	const session = logs.ok ? (logs.sessions || [])[0] : null;

	let lines = [];
	if (session) {
		const read = await api.logs.read(session.name, 40);
		if (read.ok) lines = read.session.lines;
	}

	// The shell hands the code over as an unsigned value, so a killed game arrived as 4294967295
	// while its own log said -1. `| 0` reads it back as the signed 32-bit code Windows reports.
	const code = exitCode === null || exitCode === undefined ? 0 : Number(exitCode) | 0;

	const settings = state.settings;
	const text = [
		'Aether Launcher — report',
		'',
		`problem     ${problem}`,
		code ? `exit code   ${code}` : null,
		`version     ${settings.gameVersion}`,
		`loader      ${settings.loaderVersion}`,
		`java        ${settings.javaPath || 'detected automatically'}`,
		`session     ${session ? session.name : 'no session file'}`,
		'',
		'last lines',
		...(lines.length ? lines : ['(the session log is empty)'])
	]
		.filter((line) => line !== null)
		.join('\n');

	openModal(title, `
		<p class="muted">The launcher wrote this report from the last session. Send it when you ask
		for help.</p>
		<pre class="console console--tall" id="crash-report"></pre>
		<p><button class="btn btn--ghost" id="report-open-logs" type="button">Open the logs</button></p>
	`, async () => {
		const copied = await api.copy(text);
		toast(
			copied && copied.ok ? 'The report was copied.' : 'The report could not be copied.',
			copied && copied.ok ? 'success' : 'error'
		);
	}, 'Copy report');

	$('#crash-report').textContent = text;

	// The report is a summary, the whole session is one click away.
	$('#report-open-logs').addEventListener('click', () => {
		closeModal();
		showPage('logs');
		if (session) openSession(session);
	});
}

/**
 * What a player can send when an installation fails.
 *
 * The toast alone says nothing about what was downloaded or refused; the installation log already
 * holds one line per file. The report keeps the same shape as the crash report, and the files that
 * did arrive are kept for the next attempt.
 */
async function showInstallReport(problem) {
	const logs = await api.logs.tail(40);
	const tail = logs.ok ? logs.install : null;
	const lines = tail && tail.exists ? tail.lines : [];

	const text = [
		'Aether Launcher — installation report',
		'',
		`problem     ${problem}`,
		`version     ${$('#select-version').value}`,
		`loader      ${$('#select-loader').value}`,
		`log         ${tail ? tail.path : 'install.log'}`,
		'',
		'last lines',
		...(lines.length ? lines : ['(the installation log is empty)'])
	].join('\n');

	openModal('The installation failed', `
		<p class="muted">The launcher wrote this report from the installation log. Send it when you
		ask for help. The files already downloaded are kept, the next installation resumes them.</p>
		<pre class="console console--tall" id="install-report"></pre>
		<p><button class="btn btn--ghost" id="report-open-logs" type="button">Open the logs</button></p>
	`, async () => {
		const copied = await api.copy(text);
		toast(
			copied && copied.ok ? 'The report was copied.' : 'The report could not be copied.',
			copied && copied.ok ? 'success' : 'error'
		);
	}, 'Copy report');

	$('#install-report').textContent = text;

	$('#report-open-logs').addEventListener('click', () => {
		closeModal();
		showPage('logs');
	});
}

/**
 * Tells whether an address points at a server of this computer.
 *
 * A hosted server accepts any name: the protection of the premium names only exists on public
 * servers, so warning about a loopback address would warn about nothing.
 */
function isLocalServer(address) {
	const host = String(address || '').split(':')[0].toLowerCase();

	return host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '0.0.0.0';
}

/**
 * Warns before joining a public server with a name Mojang already owns.
 *
 * A cracked server refuses such a name to protect its owner, and the only thing the game shows is
 * "Unauthorized: You cannot join the server", which says nothing about the name. The launcher
 * therefore asks Mojang first, so the player can pick another name instead of guessing.
 *
 * @return false when the launch has to wait for the player to choose.
 */
async function offlineNameIsAccepted(server) {
	const account = state.accounts.accounts.find((entry) => entry.id === state.accounts.selected);
	if (!account || account.type !== 'offline') return true;

	const status = await api.accounts.checkName(account.username);
	// A lookup that failed is not a refusal: the game stays the judge, the launch goes on.
	if (!status || !status.ok || !status.premium) return true;

	openModal('This name belongs to a Mojang account', `
		<p><strong>${status.name}</strong> is registered by a Mojang account. Cracked servers
		protect those names: they refuse a cracked client that presents one, and the game only
		shows <em>“Unauthorized: You cannot join the server”</em>.</p>
		<p class="muted">Create an offline account with a name nobody owns — for example
		<strong>${status.name}9281</strong> — then join ${server} again. Single player and the
		servers you host accept any name.</p>
		<p><button class="btn btn--ghost btn--small" id="premium-continue">Join anyway</button></p>
	`, () => offlineAccountModal(), 'Choose another name');

	$('#premium-continue').onclick = () => {
		closeModal();
		launch(server, true);
	};

	return false;
}

async function launch(server, skipNameCheck = false) {
	if (state.launching) return;
	if (!state.accounts.selected) {
		toast('Add an account first.', 'error');
		return;
	}

	if (server && !skipNameCheck && !isLocalServer(server)) {
		const accepted = await offlineNameIsAccepted(server);
		if (!accepted) return;
	}

	state.launching = true;
	setStatus(server ? `Joining ${server}…` : 'Launching…', 'busy');
	$('#btn-launch').disabled = true;
	$('#btn-stop').hidden = false;
	clearSession();

	const result = await withBusy(
		() => {
			state.launching = false;
			$('#btn-launch').disabled = false;
			$('#btn-stop').hidden = true;
		},
		() =>
			api.launch({
				accountId: state.accounts.selected,
				gameVersion: $('#select-version').value,
				server: server || ''
			})
	);

	// The answer arrives when the game closes, not when it starts.
	if (!result) return;

	if (result.stopped) {
		setStatus('Ready');
		toast('The game was stopped.', 'success');
		return;
	}

	if (!result.ok) {
		setStatus('Error', 'error');
		toast(result.error, 'error');

		// A core that died without an answer is a crash, a request it refused is not.
		if (result.crashed) {
			loadLogs();
			await showCrashReport({ title: 'The launcher core stopped', problem: result.error });
		}
		return;
	}

	setStatus('Ready');
	// Same signed reading as the report: the shell sends a killed game back as 4294967295.
	const code = result.exitCode ? Number(result.exitCode) | 0 : 0;
	if (code) {
		toast(`The game stopped with the code ${code}.`, 'error');
		loadLogs();
		await showCrashReport({
			title: 'The game stopped',
			problem: `the game stopped with the code ${code}`,
			exitCode: code
		});
		return;
	}

	toast('The game closed.', 'success');
}

/** Asks the core to stop the installation that is running. */
async function cancelInstall() {
	$('#btn-cancel-install').disabled = true;

	const result = await withBusy(() => {}, () => api.installStop());

	if (!result) return;
	if (!result.ok) {
		$('#btn-cancel-install').disabled = false;
		toast(result.error || 'No installation is running.', 'error');
		return;
	}

	// The answer of the installation arrives on its own and clears the view.
	if (state.installing) setStatus('Cancelling…', 'busy');
}

async function stopGame() {
	const result = await withBusy(() => {}, () => api.stop());
	if (!result) return;
	if (!result.ok) {
		toast(result.error || 'The game is not running.', 'error');
		return;
	}

	setStatus('Stopping…', 'busy');
}

/* -------------------------------------------------------------------- init */

/** The filled part of a slider follows its value. */
function paintRange(input) {
	const min = Number(input.min) || 0;
	const max = Number(input.max) || 100;
	const span = max - min;
	const percent = span > 0 ? ((Number(input.value) - min) / span) * 100 : 0;
	input.style.setProperty('--fill', `${percent}%`);
}

function paintRanges() {
	$$('input[type="range"]').forEach(paintRange);
}

function bindSettings() {
	const maxMemory = $('#range-max-memory');
	const minMemory = $('#range-min-memory');

	// Every slider of the launcher paints itself, wherever it lives.
	$$('input[type="range"]').forEach((input) => {
		input.addEventListener('input', () => paintRange(input));
	});

	maxMemory.addEventListener('input', () => {
		$('#output-max-memory').textContent = `${maxMemory.value} MB`;
	});
	minMemory.addEventListener('input', () => {
		$('#output-min-memory').textContent = `${minMemory.value} MB`;
	});
	// The settings page edits the default profile, the play page has to show the new values.
	maxMemory.addEventListener('change', async () => {
		await api.settings.update({ maxMemory: Number(maxMemory.value) });
		await loadProfiles();
	});
	minMemory.addEventListener('change', async () => {
		await api.settings.update({ minMemory: Number(minMemory.value) });
		await loadProfiles();
	});

	$('#range-memory').addEventListener('input', (event) => {
		$('#output-memory').textContent = `${event.target.value} MB`;
	});
	$('#range-memory').addEventListener('change', (event) => {
		updateProfile({ memory: Number(event.target.value) });
	});

	$('#range-width').addEventListener('input', (event) => {
		$('#output-width').textContent = event.target.value;
	});
	$('#range-height').addEventListener('input', (event) => {
		$('#output-height').textContent = event.target.value;
	});
	$('#range-width').addEventListener('change', (event) => {
		updateProfile({ width: Number(event.target.value) });
	});
	$('#range-height').addEventListener('change', (event) => {
		updateProfile({ height: Number(event.target.value) });
	});

	$('#input-java').addEventListener('change', (event) => {
		api.settings.update({ javaPath: event.target.value.trim() });
	});

	$('#input-accent').addEventListener('input', (event) => {
		$('#output-accent').textContent = event.target.value.toUpperCase();
		applyAccent(event.target.value);
	});
	$('#input-accent').addEventListener('change', (event) => {
		saveAppearance({ accentColor: event.target.value });
	});

	$('#theme-choices').addEventListener('click', (event) => {
		const button = event.target.closest('[data-theme]');
		if (button) saveAppearance({ theme: button.dataset.theme });
	});
	$('#surface-choices').addEventListener('click', (event) => {
		const button = event.target.closest('[data-surface]');
		if (button) saveAppearance({ surfaceStyle: button.dataset.surface });
	});
	$('#density-choices').addEventListener('click', (event) => {
		const button = event.target.closest('[data-density]');
		if (button) saveAppearance({ uiDensity: button.dataset.density });
	});
	$('#navigation-choices').addEventListener('click', (event) => {
		const button = event.target.closest('[data-navigation]');
		if (button) saveAppearance({ navigationStyle: button.dataset.navigation });
	});
	$('#appearance-reset').addEventListener('click', async () => {
		const confirmed = await confirmAction(
			'Reset workspace design?',
			'The colourway, custom accent, surface, spacing and navigation will return to their defaults.'
		);
		if (confirmed) {
			await saveAppearance({ ...APPEARANCE_DEFAULTS });
			toast('Workspace design restored.', 'success');
		}
	});

	$('#input-fabric-api').addEventListener('change', (event) => {
		api.settings.update({ fabricApiVersion: event.target.value.trim() });
	});
}

function applySettings(settings) {
	state.settings = settings;

	$('#range-memory').value = settings.maxMemory;
	$('#output-memory').textContent = `${settings.maxMemory} MB`;
	$('#range-max-memory').value = settings.maxMemory;
	$('#output-max-memory').textContent = `${settings.maxMemory} MB`;
	$('#range-min-memory').value = settings.minMemory;
	$('#output-min-memory').textContent = `${settings.minMemory} MB`;
	$('#range-width').value = settings.windowWidth;
	$('#output-width').textContent = settings.windowWidth;
	$('#range-height').value = settings.windowHeight;
	$('#output-height').textContent = settings.windowHeight;
	$('#input-java').value = settings.javaPath || '';
	$('#input-accent').value = settings.accentColor;
	$('#output-accent').textContent = settings.accentColor.toUpperCase();
	$('#input-fabric-api').value = settings.fabricApiVersion;
	$('#check-g1').checked = settings.useG1gc !== false;

	applyAppearance(settings);
	paintRanges();
}

/**
 * The colours the canvas of the HUD reads, taken from the theme instead of being written here.
 *
 * Reading them once keeps the render loop free of a style recalculation on every frame; the cache
 * is dropped when the accent changes.
 */
let themeCache = null;

function themeColors() {
	if (!themeCache) {
		const styles = getComputedStyle(document.documentElement);
					themeCache = {
				// The curve is data, not an action: it stays graphite so the accent keeps meaning
				// "primary action" on the page.
				graph: styles.getPropertyValue('--color-text-primary').trim() || '#f2f4f6',
			};
	}

	return themeCache;
}

/**
 * Applies the accent chosen by the player.
 *
 * The theme reads `--accent-user`, not `--accent`: `--accent` is the alias
 * `var(--accent-user, var(--color-accent))`, so a player who never picked a colour keeps the
 * identity of `tokens.css`. The two values below were the default accent of older builds; a
 * settings file that still carries one of them was never a choice, and it must not freeze the
 * interface on the old palette.
 */
const LEGACY_ACCENT_DEFAULTS = new Set(['#6c5ce7', '#7c5cfc', '#3b82f6', '#e7e9ec']);

function applyAccent(color) {
	const value = String(color ?? '').trim();
	const root = document.documentElement.style;

	themeCache = null;

	if (!value || LEGACY_ACCENT_DEFAULTS.has(value.toLowerCase())) {
		root.removeProperty('--accent-user');
		root.removeProperty('--on-accent');
		return;
	}

	root.setProperty('--accent-user', value);
	root.setProperty('--on-accent', inkFor(value));
}

const APPEARANCE_DEFAULTS = Object.freeze({
	theme: 'graphite',
	accentColor: '#E7E9EC',
	uiDensity: 'roomy',
	surfaceStyle: 'contour',
	navigationStyle: 'wide'
});

const APPEARANCE_THEMES = Object.freeze({
	graphite: 'GRAPHITE',
	oxide: 'OXIDE',
	field: 'FIELD',
	blueprint: 'BLUEPRINT'
});

function setChoiceState(selector, attribute, selected) {
	$(selector).forEach((button) => {
		const active = button.dataset[attribute] === selected;
		button.classList.toggle('is-active', active);
		button.setAttribute('aria-pressed', String(active));
	});
}

/** Applies the complete workspace identity without reloading the page. */
function applyAppearance(settings = {}) {
	const theme = Object.hasOwn(APPEARANCE_THEMES, settings.theme) ? settings.theme : APPEARANCE_DEFAULTS.theme;
	const density = ['roomy', 'compact'].includes(settings.uiDensity) ? settings.uiDensity : APPEARANCE_DEFAULTS.uiDensity;
	const surface = ['contour', 'draft', 'plain'].includes(settings.surfaceStyle) ? settings.surfaceStyle : APPEARANCE_DEFAULTS.surfaceStyle;
	const navigation = ['wide', 'rail'].includes(settings.navigationStyle) ? settings.navigationStyle : APPEARANCE_DEFAULTS.navigationStyle;

	const root = document.documentElement;
	root.dataset.theme = theme;
	root.dataset.density = density;
	root.dataset.surface = surface;
	root.dataset.navigation = navigation;

	setChoiceState('#theme-choices [data-theme]', 'theme', theme);
	setChoiceState('#surface-choices [data-surface]', 'surface', surface);
	setChoiceState('#density-choices [data-density]', 'density', density);
	setChoiceState('#navigation-choices [data-navigation]', 'navigation', navigation);

	const themeName = $('#studio-theme-name');
	if (themeName) themeName.textContent = `${APPEARANCE_THEMES[theme]} / ${surface.toUpperCase()}`;
	const densityName = $('#studio-preview-density');
	if (densityName) densityName.textContent = `${density.toUpperCase()} · ${navigation === 'wide' ? 'WIDE NAVIGATION' : 'ICON RAIL'}`;
	const memory = $('#studio-preview-memory');
	if (memory) {
		const amount = Number(settings.maxMemory) || 4096;
		memory.innerHTML = `${amount} <i>MB</i>`;
	}

	if ($('#input-accent')) {
		const accent = /^#[0-9a-f]{6}$/i.test(String(settings.accentColor || ''))
			? settings.accentColor
			: APPEARANCE_DEFAULTS.accentColor;
		$('#input-accent').value = accent;
		$('#output-accent').textContent = accent.toUpperCase();
		applyAccent(accent);
	}
}

/** Saves an appearance change, keeping the live preview and persisted settings in sync. */
async function saveAppearance(patch) {
	const previous = { ...state.settings };
	const optimistic = { ...previous, ...patch };
	applyAppearance(optimistic);

	try {
		const result = await api.settings.update(patch);
		if (!result || !result.ok) {
			state.settings = previous;
			applyAppearance(previous);
			toast(result?.error || 'The appearance could not be saved.', 'error');
			return false;
		}
		state.settings = { ...optimistic, ...(result.settings || {}) };
		applyAppearance(state.settings);
		return true;
	} catch (error) {
		state.settings = previous;
		applyAppearance(previous);
		toast(`The appearance could not be saved: ${error.message}`, 'error');
		return false;
	}
}

/**
 * The ink that stays readable on a given colour: white while it clears the 4.5:1 floor of the WCAG,
 * black below it. Every coloured label of the launcher reads its ink here.
 */
function contrastInk([red, green, blue]) {
	const linear = (value) => {
		const unit = value / 255;
		return unit <= 0.03928 ? unit / 12.92 : ((unit + 0.055) / 1.055) ** 2.4;
	};
	const luminance = 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue);

	return 1.05 / (luminance + 0.05) >= 4.5 ? '#ffffff' : '#000000';
}

/** The ink of a label sitting on the accent, read on the fill that `--accent-fill` paints. */
function inkFor(hex) {
	const digits = String(hex).trim().replace('#', '');
	if (!/^[0-9a-f]{6}$/i.test(digits)) return '#ffffff';

	return contrastInk([0, 2, 4].map((offset) => parseInt(digits.slice(offset, offset + 2), 16) * 0.82));
}

async function loadLoaders(gameVersion) {
	const result = await api.versions.loaders(gameVersion);
	state.loaders = result.ok ? result.loaders : [];
	renderLoaders();
}

/** Creates a profile from the one the selected account plays with right now. */
function newProfileModal() {
	openModal('New profile', `
		<p class="muted">The new profile copies the one this account plays with right now: version,
		loader, memory and window size.</p>
		<label class="field">
			<span class="field-label">Profile name</span>
			<input type="text" id="profile-name" class="input" maxlength="32" placeholder="Cinematic" />
		</label>
	`, async () => {
		const name = $('#profile-name').value.trim();
		if (!name) {
			toast('Name the profile first.', 'error');
			return;
		}

		const source = state.profiles.activeFor;
		const result = await api.profiles.add(name, {
			version: source.gameVersion,
			loader: source.loaderVersion,
			memory: source.maxMemory,
			minMemory: source.minMemory,
			width: source.windowWidth,
			height: source.windowHeight
		});

		if (!result.ok) {
			toast(result.error, 'error');
			return;
		}

		await loadProfiles();
		toast(`Profile ${name} created.`, 'success');
	}, 'Create');
}

/** Deletes the profile shown in the selector, once the player confirmed it. */
function deleteProfile() {
	const name = $('#select-profile').value;
	if (!name) return;

	openModal('Delete profile', `
		<p class="muted">Delete the profile <strong id="profile-to-delete"></strong>? The version, the
		loader, the memory and the window size of that profile are lost. An account that plays with
		it falls back to the active profile.</p>
	`, async () => {
		const result = await api.profiles.remove(name);

		if (!result.ok) {
			toast(result.error, 'error');
			return;
		}

		// The accounts that had chosen it fell back to the active profile.
		state.accounts = await api.accounts.get();
		renderAccounts();
		renderAccountTable();
		await loadProfiles();
		toast(`Profile ${name} removed.`, 'success');
	}, 'Delete');

	$('#profile-to-delete').textContent = name;
}

/** The profile the selected account plays with: the values the play controls show. */
function currentProfile() {
	return state.profiles.activeFor || {
		name: 'Default',
		gameVersion: state.settings.gameVersion,
		loaderVersion: state.settings.loaderVersion,
		maxMemory: state.settings.maxMemory,
		minMemory: state.settings.minMemory,
		windowWidth: state.settings.windowWidth,
		windowHeight: state.settings.windowHeight
	};
}

/** Selects a value, adding the option when the list does not offer it yet. */
/**
 * The dock is the only place that launches the game: it shows the account and the profile the
 * button will use, whatever page is open.
 */
function renderDock() {
	const account = state.accounts.accounts.find((entry) => entry.id === state.accounts.selected);
	const profile = currentProfile();

	const avatar = $('#dock-avatar');
	if (avatar) {
		avatar.textContent = account ? account.username.charAt(0).toUpperCase() : '?';
		avatar.style.backgroundImage = '';
		if (account) paintAvatar(avatar, account.username);
	}

	const name = $('#dock-account-name');
	if (name) name.textContent = account ? account.username : 'No account';

	const version = $('#dock-version');
	if (version) {
		version.textContent = profile.loaderVersion
			? `${profile.gameVersion} · Fabric ${profile.loaderVersion}`
			: profile.gameVersion;
	}

	const memory = $('#dock-memory');
	if (memory) memory.textContent = `${profile.maxMemory} MB`;
}

function selectOption(select, value) {
	if (!value) return;

	if (!Array.from(select.options).some((option) => option.value === value)) {
		const option = document.createElement('option');
		option.value = value;
		option.textContent = value;
		select.appendChild(option);
	}

	select.value = value;
}

/** Shows the values of a profile in the play controls. */
function applyProfile(profile) {
	$('#range-memory').value = profile.maxMemory;
	$('#output-memory').textContent = `${profile.maxMemory} MB`;
	$('#range-width').value = profile.windowWidth;
	$('#output-width').textContent = profile.windowWidth;
	$('#range-height').value = profile.windowHeight;
	$('#output-height').textContent = profile.windowHeight;

	selectOption($('#select-version'), profile.gameVersion);
	selectOption($('#select-loader'), profile.loaderVersion);

	paintRanges();
	renderDock();
}

/**
 * Writes the play controls into the profile of the account, never into the flat settings: two
 * accounts with two profiles must be able to play two different configurations.
 */
async function updateProfile(patch) {
	const result = await api.profiles.update(currentProfile().name, patch);

	if (!result.ok) {
		toast(result.error, 'error');
		await loadProfiles();
		return;
	}

	await loadProfiles();
}

/** The named profiles and the one that applies to the selected account. */
async function loadProfiles() {
	const result = await api.profiles.list();
	if (!result.ok) return;

	state.profiles = result;

	const select = $('#select-profile');
	select.innerHTML = '';

	for (const profile of result.profiles) {
		const option = document.createElement('option');
		option.value = profile.name;
		option.textContent = `${profile.name} · ${profile.gameVersion} · ${profile.maxMemory} MB`;
		option.selected = result.activeFor.name === profile.name;
		select.appendChild(option);
	}

	applyProfile(result.activeFor);
}

/**
 * Opens a page by its name, exactly like a click on the navigation does.
 *
 * The reports use it to bring the player to the logs instead of only telling them where the
 * files are.
 */
function showPage(name) {
	let activeBtn = null;
	$$('.nav-item').forEach((other) => {
		const match = other.dataset.page === name;
		other.classList.toggle('is-active', match);
		if (match) activeBtn = other;
	});

	// Smooth sliding nav indicator with GPU transform
	const indicator = $('#nav-indicator');
	if (indicator && activeBtn) {
		const sidebarTop = $('.sidebar').getBoundingClientRect().top;
		const btnRect = activeBtn.getBoundingClientRect();
		const targetY = btnRect.top - sidebarTop + (btnRect.height - 28) / 2;
		indicator.style.transform = `translateY(${Math.round(targetY)}px)`;
		indicator.classList.add('is-visible');
	}

	$$('.page').forEach((page) => page.classList.remove('is-active'));
	$(`.page[data-page="${name}"]`).classList.add('is-active');

	// A page opens at its top, a scroll of the previous one is not kept.
	$('.content').scrollTop = 0;

	// Lazy load views & subdata
	if (name === 'mods') loadMods();
	if (name === 'logs') loadLogs();
	if (name === 'servers') {
		loadServers();
		loadHosts();
	}
	if (name === 'performance') {
		loadPerformance();
		setTimeout(initFpsSplineGraph, 50);
	}
}

async function init() {
	// Window controls.
	$$('[data-window]').forEach((button) => {
		button.addEventListener('click', () => api.window(button.dataset.window));
	});

	// Navigation.
	$$('.nav-item').forEach((item) => {
		item.addEventListener('click', () => showPage(item.dataset.page));
	});

	$('#modal-cancel').addEventListener('click', closeModal);
	$('#modal').addEventListener('click', (event) => {
		if (event.target === $('#modal')) closeModal();
	});

	// Escape closes the dialog, Tab stays inside it while it is open.
	document.addEventListener('keydown', (event) => {
		if ($('#modal').hidden) return;

		if (event.key === 'Escape') {
			event.preventDefault();
			closeModal();
			return;
		}

		if (event.key !== 'Tab') return;

		const fields = $$('#modal input, #modal select, #modal textarea, #modal button').filter(
			(element) => !element.disabled && element.offsetParent !== null
		);
		if (!fields.length) return;

		const first = fields[0];
		const last = fields[fields.length - 1];

		if (event.shiftKey && document.activeElement === first) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && document.activeElement === last) {
			event.preventDefault();
			first.focus();
		}
	});

	$('#btn-add-offline').addEventListener('click', offlineAccountModal);
	$('#play-add-offline').addEventListener('click', offlineAccountModal);
	$('#btn-add-microsoft').addEventListener('click', microsoftAccountModal);
	$('#btn-install').addEventListener('click', install);
	$('#btn-cancel-install').addEventListener('click', cancelInstall);
	$('#btn-launch').addEventListener('click', () => launch());
	$('#btn-stop').addEventListener('click', stopGame);
	$('#btn-refresh-versions').addEventListener('click', loadVersions);
	$('#btn-mods-refresh').addEventListener('click', loadMods);
	$('#btn-mods-folder').addEventListener('click', openModsFolder);
	$('#btn-logs-refresh').addEventListener('click', loadLogs);

	// The servers and the performance pages.
	$('#btn-server-add').addEventListener('click', addServer);
	$('#btn-server-probe').addEventListener('click', () => testAddress());
	$('#btn-servers-refresh').addEventListener('click', () => {
		loadServers();
		loadHosts();
	});
	$('#btn-host-create').addEventListener('click', createHost);
	$('#btn-perf-refresh').addEventListener('click', loadPerformance);
	$('#btn-perf-install').addEventListener('click', installPack);
	$('#btn-jvm-apply').addEventListener('click', applyJvm);
	$('#btn-tune-max').addEventListener('click', () => tuneOptions('max'));
	$('#btn-tune-balanced').addEventListener('click', () => tuneOptions('balanced'));
	$('#btn-tune-restore').addEventListener('click', () => tuneOptions('restore'));
	$('#server-address').addEventListener('keydown', (event) => {
		if (event.key === 'Enter') addServer();
	});
	$('#host-name').addEventListener('keydown', (event) => {
		if (event.key === 'Enter') createHost();
	});

	$('#check-g1').addEventListener('change', (event) => {
		api.settings.update({ useG1GC: event.target.checked });
	});

	$('#select-version').addEventListener('change', async (event) => {
		await updateProfile({ version: event.target.value });
		loadLoaders(event.target.value);
	});
	$('#select-loader').addEventListener('change', (event) => {
		updateProfile({ loader: event.target.value });
	});

	// A profile belongs to an account: changing it here changes it for the selected one.
	$('#select-profile').addEventListener('change', async (event) => {
		const account = state.accounts.selected;
		if (!account) {
			toast('Add an account first.', 'error');
			loadProfiles();
			return;
		}

		const result = await api.profiles.assign(account, event.target.value);
		if (!result.ok) {
			toast(result.error, 'error');
			loadProfiles();
			return;
		}

		await loadProfiles();
		toast(`This account now plays with ${event.target.value}.`, 'success');
	});

	$('#btn-profile-new').addEventListener('click', newProfileModal);
	$('#btn-profile-delete').addEventListener('click', deleteProfile);

	bindSettings();

	// Interactive preset cards on Performance page
	$$('.preset-card').forEach((card) => {
		card.addEventListener('click', () => {
			const preset = card.dataset.preset;
			selectOption($('#select-jvm'), preset);
			$$('.preset-card').forEach((c) => c.classList.remove('is-active'));
			card.classList.add('is-active');
		});
	});

	// Debounced window resize handler
	let resizeDebounce = null;
	window.addEventListener('resize', () => {
		clearTimeout(resizeDebounce);
		resizeDebounce = setTimeout(() => {
			const activePage = state.page;
			showPage(activePage);
		}, 150);
	});

	// The game output.
	api.onGameOutput((line) => {
		log(line);
		// The first line of the game means that Java started, the launch only answers at its end.
		if (state.launching) setStatus('Playing', 'busy');
	});
	api.onInstallProgress((progress) => {
		// A progress line can arrive after the failure that ended the installation: it must not
		// repaint a bar that is already hidden and reset.
		if (!state.installing) return;

		const percent = progress.total > 0
			? Math.min(100, Math.round((progress.received / progress.total) * 100))
			: 0;
		$('#progress-label').textContent = `${progress.task} (${percent}%)`;
		$('#progress-fill').style.width = `${percent}%`;
	});
	api.onOptimisationProgress((progress) => {
		if (!state.performance.installing) return;

		const detail = progress.detail ? ` — ${progress.detail}` : '';
		$('#perf-path').textContent = `${progress.name}: ${progress.state}${detail}`;
	});

	// The code of the sign in with Microsoft.
	api.onMicrosoftDeviceCode((code) => {
		const value = document.querySelector('#device-code');
		const link = document.querySelector('#verify-link');
		if (value) value.textContent = code.userCode;
		if (link) {
			link.textContent = code.verificationUri;
			link.href = code.verificationUri;
		}
	});

	// The initial state.
	const info = await api.info();
	if (!info.ok) {
		// Without an answer from the core there is nothing to show, the reason is reported instead
		// of a stack trace from the panel that reads it.
		$('#paths-output').textContent = info.error;
		toast(info.error, 'error');
		setStatus('Error', 'error');
		return;
	}

	applySettings(info.settings);
	$('#paths-output').textContent = [
		`game   ${info.paths.game}`,
		`mods   ${info.paths.mods}`,
		`assets ${info.paths.assets}`,
		`data   ${info.paths.data}`
	].join('\n');

	state.accounts = await api.accounts.get();
	renderAccounts();
	renderAccountTable();

	await loadVersions();
	await loadLoaders(state.settings.gameVersion);
	await loadProfiles();
	await loadMods();
	await loadLogs();
	await loadServers();
	await loadHosts();
	await loadPerformance();
}

async function loadVersions() {
	const result = await api.versions.list();
	if (!result.ok) {
		toast(result.error, 'error');
		return;
	}
	state.versions = result.versions;
	renderVersionSelect();
	renderVersions();
}

init().catch((error) => {
	toast(`The launcher could not start: ${error.message}`, 'error');
	setStatus('Error', 'error');
});
