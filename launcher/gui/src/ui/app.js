const api = window.aether;

const state = {
	page: 'play',
	accounts: { accounts: [], selected: null },
	settings: {},
	javaRuntime: null,
	versions: [],
	loaders: [],
	mods: [],
	modsLibrary: { projects: [], query: '', gameVersion: '', loading: false, searched: false, installingId: null, progress: null, requestId: 0 },
	profiles: { profiles: [], activeFor: null },
	servers: { servers: [], pings: {} },
	hosts: [],
	performance: { mods: [], state: {}, installing: false, checked: false },
	sessionLines: 0,
	installing: false,
	launching: false,
	status: 'Ready'
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
	state.status = text;
	const dashboardStatus = $('#dashboard-status');
	if (dashboardStatus) dashboardStatus.textContent = text;
}

/* ------------------------------------------------------- workspace UX */

function applyTheme(theme, persist = true) {
	const normalized = theme === 'light' ? 'light' : 'dark';
	document.documentElement.dataset.theme = normalized;
	const select = $('#select-theme');
	if (select) select.value = normalized;
	const button = $('#theme-toggle');
	if (button) {
		button.setAttribute('aria-label', normalized === 'dark' ? 'Switch to light theme' : 'Switch to dark theme');
		button.title = normalized === 'dark' ? 'Switch to light theme' : 'Switch to dark theme';
	}
	if (state.settings) state.settings.theme = normalized;
	if (persist && api?.settings?.update) {
		api.settings.update({ theme: normalized }).then((result) => {
			if (!result?.ok) toast(result?.error || 'The theme could not be saved.', 'error');
		}).catch((error) => toast(error.message || 'The theme could not be saved.', 'error'));
	}
}

function renderDashboard() {
	const account = state.accounts.accounts.find((entry) => entry.id === state.accounts.selected);
	const profile = currentProfile();
	const runtime = state.javaRuntime;
	const requiredJava = Number(runtime?.requiredMajor || state.settings.requiredJavaVersion) || 21;
	const status = state.installing ? 'Installing…' : state.launching ? 'Starting Minecraft…' : (state.status || 'Ready');
	const heroVersion = $('#hero-version');
	if (heroVersion) heroVersion.textContent = (profile?.gameVersion || 'Minecraft') + ' · FABRIC';
	$('#dashboard-account').textContent = account?.username || 'No account selected';
	$('#dashboard-account-meta').textContent = account
		? (account.type === 'offline' ? 'Offline account · Local profile' : 'Microsoft account')
		: 'Add an account to get started';
	$('#dashboard-profile').textContent = profile?.name || 'Default';
	$('#dashboard-profile-meta').textContent = profile
		? 'Minecraft ' + (profile.gameVersion || '—') + ' · ' + (Math.round(profile.maxMemory / 1024 * 10) / 10) + ' GB RAM'
		: 'Choose a profile';
	$('#dashboard-java').textContent = runtime?.compatible
		? 'Java ' + runtime.majorVersion + ' · Ready'
		: runtime?.available ? 'Java version too old' : 'Java not found';
	$('#dashboard-java-meta').textContent = runtime?.compatible
		? (runtime.source === 'custom' ? 'Custom runtime selected' : 'Automatically detected') + ' · ' + requiredJava + ' required'
		: (runtime?.source === 'custom-invalid' ? 'Check your custom java.exe path' : 'Install Java ' + requiredJava + ' or configure it in Settings');
	const runtimeStatus = $('#java-runtime-status');
	if (runtimeStatus) {
		runtimeStatus.textContent = runtime?.compatible
			? 'Using Java ' + runtime.majorVersion + ' · ' + runtime.path
			: runtime?.available
				? 'Java ' + runtime.majorVersion + ' was found, but Java ' + requiredJava + ' or newer is required.'
				: runtime?.source === 'custom-invalid'
					? 'The selected Java executable is missing, invalid or too old. Clear the field to use automatic detection.'
					: 'No compatible Java runtime was found. Install Java ' + requiredJava + ' or choose java.exe above.';
	}
	$('#dashboard-status').textContent = status;
	renderQuickServers();
}


function renderQuickServers() {
	const container = $('#quick-servers');
	if (!container) return;
	container.replaceChildren();
	const servers = (state.servers.servers || []).slice(0, 4);

	if (servers.length === 0) {
		const empty = emptyState(
			ICON_SERVER,
			'No saved servers yet',
			'Add a server once and it will be ready here next time.',
			'Browse servers'
		);
		empty.querySelector('[data-empty-action]')?.addEventListener('click', () => showPage('servers'));
		container.appendChild(empty);
		return;
	}

	for (const server of servers) {
		const ping = state.servers.pings[server.address];
		const row = document.createElement('div');
		row.className = 'quick-server-row';

		const identity = document.createElement('div');
		identity.className = 'quick-server-identity';
		const indicator = document.createElement('span');
		indicator.className = 'quick-server-indicator' + (ping?.online ? ' is-online' : '');
		const copy = document.createElement('div');
		copy.className = 'quick-server-copy';
		const name = document.createElement('strong');
		name.textContent = server.name;
		const address = document.createElement('small');
		address.textContent = server.address;
		copy.append(name, address);
		identity.append(indicator, copy);

		const meta = document.createElement('small');
		meta.className = 'quick-server-meta';
		meta.textContent = pingSummary(ping) || 'Not checked yet';

		const join = document.createElement('button');
		join.type = 'button';
		join.className = 'btn btn--primary btn--small';
		join.textContent = 'Join';
		join.disabled = !state.accounts.selected || state.launching || state.installing;
		join.title = state.accounts.selected ? 'Join ' + server.name : 'Select an account before joining';
		join.addEventListener('click', () => launch(server.address));

		row.append(identity, meta, join);
		container.appendChild(row);
	}

	if ((state.servers.servers || []).length > servers.length) {
		const more = document.createElement('button');
		more.type = 'button';
		more.className = 'quick-server-more';
		more.textContent = 'View all ' + state.servers.servers.length + ' saved servers';
		more.addEventListener('click', () => showPage('servers'));
		container.appendChild(more);
	}
}

function renderSetupChecklist() {
	const card = $('#setup-card');
	if (!card) return;
	const accountsReady = state.accounts.accounts.length > 0 && Boolean(state.accounts.selected);
	const profileReady = accountsReady && Boolean(state.profiles.activeFor);
	const mods = state.performance.mods || [];
	const installed = mods.filter((mod) => mod.installed).length;
	const performanceReady = state.performance.checked && mods.length > 0 && installed === mods.length;
	const current = currentProfile();
	const steps = [
		{ id: '#setup-account', state: '#setup-account-state', complete: accountsReady,
			done: 'Account selected and ready', pending: 'Create an offline or Microsoft account' },
		{ id: '#setup-profile', state: '#setup-profile-state', complete: profileReady,
			done: (current.name || 'Default') + ' · Minecraft ' + (current.gameVersion || '—'), pending: 'Set your Minecraft version and memory' },
		{ id: '#setup-performance', state: '#setup-performance-state', complete: performanceReady,
			done: 'Optimization pack ready · ' + installed + '/' + mods.length,
			pending: state.performance.checked ? installed + '/' + mods.length + ' optimization mods installed' : 'Review or install the optimization pack' }
	];
	for (const step of steps) {
		const button = $(step.id);
		const label = $(step.state);
		button?.classList.toggle('is-complete', step.complete);
		if (label) label.textContent = step.complete ? step.done : step.pending;
	}
}

function commandItems() {
	return [
		{ id: 'play', label: 'Play', detail: 'Open your game dashboard', icon: '▶', type: 'page' },
		{ id: 'accounts', label: 'Accounts', detail: 'Add or switch accounts', icon: '◎', type: 'page' },
		{ id: 'versions', label: 'Versions', detail: 'Browse Minecraft releases', icon: '▱', type: 'page' },
		{ id: 'mods', label: 'Mods', detail: 'Manage installed mods', icon: '⌘', type: 'page' },
		{ id: 'servers', label: 'Servers', detail: 'Check or host a server', icon: '⌁', type: 'page' },
		{ id: 'performance', label: 'Performance', detail: 'Optimization pack and JVM presets', icon: '↗', type: 'page' },
		{ id: 'logs', label: 'Logs', detail: 'Inspect game and install output', icon: '≡', type: 'page' },
		{ id: 'settings', label: 'Settings', detail: 'Runtime, memory and appearance', icon: '⚙', type: 'page' },
		{ id: 'launch', label: 'Launch Minecraft', detail: 'Start the selected profile', icon: '▶', type: 'action' },
		{ id: 'install', label: 'Install / repair game', detail: 'Install missing files for this profile', icon: '↓', type: 'action' },
		{ id: 'add-account', label: 'Add offline account', detail: 'Create a local player account', icon: '+', type: 'action' },
		{ id: 'theme', label: 'Toggle appearance', detail: 'Switch between dark and light mode', icon: '◐', type: 'action' }
	];
}

let commandSelection = 0;
let filteredCommands = [];

function renderCommandResults() {
	const query = ($('#command-search').value || '').trim().toLocaleLowerCase();
	filteredCommands = commandItems().filter((item) =>
		(item.label + ' ' + item.detail + ' ' + item.id).toLocaleLowerCase().includes(query)
	);
	commandSelection = Math.min(commandSelection, Math.max(0, filteredCommands.length - 1));
	const results = $('#command-results');
	results.replaceChildren();
	if (!filteredCommands.length) {
		const empty = document.createElement('div');
		empty.className = 'command-empty';
		empty.textContent = 'No matching pages or actions.';
		results.appendChild(empty);
		return;
	}
	filteredCommands.forEach((item, index) => {
		const button = document.createElement('button');
		button.type = 'button';
		button.className = 'command-result' + (index === commandSelection ? ' is-selected' : '');
		button.setAttribute('role', 'option');
		button.setAttribute('aria-selected', String(index === commandSelection));
		const icon = document.createElement('span');
		icon.className = 'command-result-icon';
		icon.textContent = item.icon;
		const copy = document.createElement('span');
		copy.className = 'command-result-copy';
		const title = document.createElement('strong');
		title.textContent = item.label;
		const detail = document.createElement('small');
		detail.textContent = item.detail;
		copy.append(title, detail);
		button.append(icon, copy);
		button.addEventListener('mouseenter', () => {
			commandSelection = index;
			updateCommandSelection();
		});
		button.addEventListener('click', () => runCommand(item));
		results.appendChild(button);
	});
}

function updateCommandSelection() {
	$$('.command-result').forEach((button, index) => {
		const selected = index === commandSelection;
		button.classList.toggle('is-selected', selected);
		button.setAttribute('aria-selected', String(selected));
		if (selected) button.scrollIntoView({ block: 'nearest' });
	});
}

function openCommandPalette() {
	const palette = $('#command-palette');
	if (!palette || !palette.hidden) return;
	palette.hidden = false;
	$('#command-search').value = '';
	commandSelection = 0;
	renderCommandResults();
	// Focus synchronously: Ctrl+K should be ready for typing on the same interaction.
	$('#command-search').focus({ preventScroll: true });
}

function closeCommandPalette() {
	const palette = $('#command-palette');
	if (!palette || palette.hidden) return;
	palette.hidden = true;
	$('#command-open').focus();
}

function runCommand(item) {
	closeCommandPalette();
	if (item.type === 'page') showPage(item.id);
	else if (item.id === 'launch') launch();
	else if (item.id === 'install') install();
	else if (item.id === 'add-account') {
		showPage('accounts');
		offlineAccountModal();
	} else if (item.id === 'theme') {
		applyTheme(document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark');
	}
}

function initWorkspaceControls() {
	try {
		document.body.classList.toggle('sidebar-collapsed', localStorage.getItem('aether.sidebar.collapsed') === '1');
	} catch { /* local storage can be disabled by the host */ }
	$('#sidebar-toggle')?.setAttribute('aria-pressed', String(document.body.classList.contains('sidebar-collapsed')));
	$$('.nav-item').forEach((button) => { button.dataset.tooltip = button.textContent.trim(); });
	$('#sidebar-toggle')?.addEventListener('click', () => {
		const collapsed = document.body.classList.toggle('sidebar-collapsed');
		$('#sidebar-toggle').setAttribute('aria-pressed', String(collapsed));
		try { localStorage.setItem('aether.sidebar.collapsed', collapsed ? '1' : '0'); } catch { /* optional preference */ }
	});
	$('#theme-toggle')?.addEventListener('click', () => {
		applyTheme(document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark');
	});
	$('#command-open')?.addEventListener('click', openCommandPalette);
	$('#command-search')?.addEventListener('input', () => {
		commandSelection = 0;
		renderCommandResults();
	});
	$('#command-palette')?.addEventListener('click', (event) => {
		if (event.target === $('#command-palette')) closeCommandPalette();
	});
	$('#setup-dismiss')?.addEventListener('click', () => {
		$('#setup-card').hidden = true;
		try { localStorage.setItem('aether.setup.dismissed', '1'); } catch { /* optional preference */ }
	});
	$('#setup-account')?.addEventListener('click', () => showPage('accounts'));
	$('#setup-profile')?.addEventListener('click', () => showPage('play'));
	$('#setup-performance')?.addEventListener('click', () => showPage('performance'));
	try {
		if (localStorage.getItem('aether.setup.dismissed') === '1') $('#setup-card').hidden = true;
	} catch { /* optional preference */ }

	document.addEventListener('keydown', (event) => {
		if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
			event.preventDefault();
			openCommandPalette();
			return;
		}
		if (!$('#command-palette').hidden) {
			if (event.key === 'Escape') {
				event.preventDefault();
				closeCommandPalette();
			} else if (event.key === 'ArrowDown' && filteredCommands.length) {
				event.preventDefault();
				commandSelection = (commandSelection + 1) % filteredCommands.length;
				updateCommandSelection();
			} else if (event.key === 'ArrowUp' && filteredCommands.length) {
				event.preventDefault();
				commandSelection = (commandSelection - 1 + filteredCommands.length) % filteredCommands.length;
				updateCommandSelection();
			} else if (event.key === 'Enter' && filteredCommands[commandSelection]) {
				event.preventDefault();
				runCommand(filteredCommands[commandSelection]);
			}
			return;
		}
		if (!$('#modal').hidden && event.key !== 'Escape') return;
		if ((event.ctrlKey || event.metaKey) && /^[1-8]$/.test(event.key)) {
			const pages = ['play', 'accounts', 'versions', 'mods', 'servers', 'performance', 'logs', 'settings'];
			event.preventDefault();
			showPage(pages[Number(event.key) - 1]);
		}
	});
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

let skinCanvasAnimId = null;
let skinRotationAngle = 0;
let skinIsDragging = false;
let skinDragStartX = 0;

function init3DSkinViewer() {
	const canvas = $('#player-skin-canvas');
	if (!canvas) return;
	const ctx = canvas.getContext('2d');
	if (!ctx) return;

	canvas.addEventListener('mousedown', (e) => {
		skinIsDragging = true;
		skinDragStartX = e.clientX;
	});

	window.addEventListener('mousemove', (e) => {
		if (!skinIsDragging) return;
		const delta = e.clientX - skinDragStartX;
		skinRotationAngle += delta * 0.02;
		skinDragStartX = e.clientX;
	});

	window.addEventListener('mouseup', () => {
		skinIsDragging = false;
	});

	function drawIsometricBox(cx, cy, w, h, d, angle, colTop, colFront, colSide) {
		const cos = Math.cos(angle);
		const sin = Math.sin(angle);

		ctx.save();
		ctx.translate(cx, cy);

		const frontX1 = -w * cos;
		const frontY1 = -w * sin * 0.45;
		const frontX2 = w * cos;
		const frontY2 = w * sin * 0.45;

		const sideX1 = -d * sin;
		const sideY1 = d * cos * 0.45;
		const sideX2 = d * sin;
		const sideY2 = -d * cos * 0.45;

		// Side Face
		ctx.fillStyle = colSide;
		ctx.beginPath();
		ctx.moveTo(frontX2, frontY2);
		ctx.lineTo(frontX2 + sideX1, frontY2 + sideY1);
		ctx.lineTo(frontX2 + sideX1, frontY2 + sideY1 + h);
		ctx.lineTo(frontX2, frontY2 + h);
		ctx.closePath();
		ctx.fill();

		// Front Face
		ctx.fillStyle = colFront;
		ctx.beginPath();
		ctx.moveTo(frontX1, frontY1);
		ctx.lineTo(frontX2, frontY2);
		ctx.lineTo(frontX2, frontY2 + h);
		ctx.lineTo(frontX1, frontY1 + h);
		ctx.closePath();
		ctx.fill();

		// Top Face
		ctx.fillStyle = colTop;
		ctx.beginPath();
		ctx.moveTo(0, -w * Math.abs(sin) * 0.5 - d * Math.abs(cos) * 0.5);
		ctx.lineTo(frontX2, frontY2);
		ctx.lineTo(frontX2 + sideX1, frontY2 + sideY1);
		ctx.lineTo(frontX1 + sideX1, frontY1 + sideY1);
		ctx.closePath();
		ctx.fill();

		ctx.restore();
	}

	function renderFrame() {
		ctx.clearRect(0, 0, canvas.width, canvas.height);

		const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
		if (!skinIsDragging && !reduceMotion) {
			skinRotationAngle += 0.012; // Gentle idle spin
		}

		const selected = state.accounts.accounts.find((a) => a.id === state.accounts.selected);
		const baseHue = selected ? (selected.type === 'offline' ? 260 : 200) : 260;

		const colSkinTop   = `hsl(${baseHue}, 50%, 68%)`;
		const colSkinFront = `hsl(${baseHue}, 65%, 52%)`;
		const colSkinSide  = `hsl(${baseHue}, 70%, 38%)`;

		const colBodyTop   = `hsl(${baseHue + 20}, 45%, 45%)`;
		const colBodyFront = `hsl(${baseHue + 20}, 55%, 35%)`;
		const colBodySide  = `hsl(${baseHue + 20}, 60%, 25%)`;

		// Head
		drawIsometricBox(60, 34, 15, 20, 15, skinRotationAngle, colSkinTop, colSkinFront, colSkinSide);
		// Torso
		drawIsometricBox(60, 60, 16, 26, 12, skinRotationAngle, colBodyTop, colBodyFront, colBodySide);
		// Left/Right Arm
		const armSwing = Math.sin(skinRotationAngle * 2) * 3;
		drawIsometricBox(38, 62 + armSwing, 7, 24, 7, skinRotationAngle, colSkinTop, colSkinFront, colSkinSide);
		drawIsometricBox(82, 62 - armSwing, 7, 24, 7, skinRotationAngle, colSkinTop, colSkinFront, colSkinSide);

		const playPage = canvas.closest('.page');
		if (!document.hidden && !reduceMotion && playPage?.classList.contains('is-active')) {
			skinCanvasAnimId = requestAnimationFrame(renderFrame);
		} else {
			skinCanvasAnimId = null;
		}
	}

	if (skinCanvasAnimId) cancelAnimationFrame(skinCanvasAnimId);
	skinCanvasAnimId = requestAnimationFrame(renderFrame);
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

function avatarColor(name) {
	let hash = 0;
	for (let i = 0; i < name.length; i++) {
		hash = name.charCodeAt(i) + ((hash << 5) - hash);
	}
	const hue = Math.abs(hash) % 360;
	return `hsl(${hue}, 62%, 52%)`;
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
			avatar.style.background = avatarColor(account.username);
			avatar.textContent = account.username.charAt(0);

			const meta = document.createElement('div');
			meta.className = 'account-meta';
			const name = document.createElement('div');
			name.className = 'account-name';
			name.textContent = account.username;
			const uuid = document.createElement('div');
			uuid.className = 'account-uuid';
			uuid.textContent = account.uuid;
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
		avatar.style.background = avatarColor(account.username);
		avatar.textContent = account.username.charAt(0);

		const meta = document.createElement('div');
		meta.className = 'account-meta';
		const name = document.createElement('div');
		name.className = 'account-name';
		name.textContent = account.username;
		const uuid = document.createElement('div');
		uuid.className = 'account-uuid';
		uuid.textContent = account.uuid;
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
			await loadProfiles();
			renderDashboard();
			renderSetupChecklist();
			toast(`Removed ${account.username}`, 'success');
		});

		row.append(avatar, meta, tag, select, remove);
		table.appendChild(row);
	}
}

function renderVersions() {
	const table = $('#version-table');
	table.innerHTML = '';

	const query = ($('#versions-search')?.value || '').trim().toLocaleLowerCase();
	const type = $('#versions-filter')?.value || 'all';
	const filtered = state.versions.filter((version) => {
		const matchesQuery = String(version.id || '').toLocaleLowerCase().includes(query);
		const matchesType = type === 'all' || version.type === type;
		return matchesQuery && matchesType;
	});
	const summary = $('#versions-summary');
	if (summary) summary.textContent = state.versions.length
		? `Showing ${filtered.length} of ${state.versions.length} Mojang versions`
		: 'The Minecraft version list is not available yet.';

	if (state.versions.length === 0) {
		table.appendChild(emptyState(
			ICON_STACK,
			'Reading the version list…',
			'The versions are published by Mojang, the list fills itself as soon as the answer arrives.'
		));
		return;
	}

	if (filtered.length === 0) {
		table.appendChild(emptyState(
			ICON_STACK,
			'No matching versions',
			'Try another version name or change the type filter.'
		));
		return;
	}

	for (const version of filtered) {
		const row = document.createElement('div');
		row.className = 'version-row';

		const left = document.createElement('div');
		const id = document.createElement('div');
		id.className = 'version-id';
		id.textContent = version.id;
		const date = document.createElement('div');
		date.className = 'version-date';
		const releaseDate = version.releaseTime ? new Date(version.releaseTime).toLocaleDateString() : 'Release date unavailable';
		date.textContent = releaseDate + (version.type ? ' · ' + version.type.replace('_', ' ') : '');
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

function setModsTab(tab) {
	const discover = tab === 'discover';
	$('#mods-installed-panel').hidden = discover;
	$('#mods-discover-panel').hidden = !discover;
	$('#mods-tab-installed').classList.toggle('is-active', !discover);
	$('#mods-tab-discover').classList.toggle('is-active', discover);
	$('#mods-tab-installed').setAttribute('aria-selected', String(!discover));
	$('#mods-tab-discover').setAttribute('aria-selected', String(discover));
	if (discover && !state.modsLibrary.searched && !state.modsLibrary.loading) {
		searchModrinth();
	}
}

function renderModrinthProjects() {
	const container = $('#mods-library-results');
	if (!container) return;
	container.replaceChildren();

	const projects = state.modsLibrary.projects || [];
	if (state.modsLibrary.loading && projects.length === 0) {
		const loading = document.createElement('div');
		loading.className = 'mods-library-empty';
		loading.textContent = 'Searching compatible Fabric mods…';
		container.appendChild(loading);
		return;
	}
	if (projects.length === 0) {
		const empty = document.createElement('div');
		empty.className = 'mods-library-empty';
		empty.textContent = state.modsLibrary.searched
			? 'No compatible mods found. Try another search.'
			: 'Search for a mod to get started.';
		container.appendChild(empty);
		return;
	}

	for (const project of projects) {
		const card = document.createElement('article');
		card.className = 'mods-library-item';

		const top = document.createElement('div');
		top.className = 'mods-library-item-top';
		const identity = document.createElement('div');
		identity.className = 'mods-library-identity';

		if (project.iconUrl && project.iconUrl.startsWith('https://cdn.modrinth.com/')) {
			const icon = document.createElement('img');
			icon.className = 'mods-library-icon';
			icon.src = project.iconUrl;
			icon.alt = '';
			icon.loading = 'lazy';
			icon.referrerPolicy = 'no-referrer';
			icon.addEventListener('error', () => icon.remove(), { once: true });
			identity.appendChild(icon);
		} else {
			const fallback = document.createElement('span');
			fallback.className = 'mods-library-icon mods-library-icon-fallback';
			fallback.textContent = (project.title || 'M').slice(0, 1).toUpperCase();
			identity.appendChild(fallback);
		}

		const title = document.createElement('div');
		title.className = 'mods-library-title-wrap';
		const name = document.createElement('strong');
		name.className = 'mods-library-name';
		name.textContent = project.title;
		const author = document.createElement('small');
		author.textContent = 'by ' + (project.author || 'Unknown author');
		title.append(name, author);
		identity.appendChild(title);
		top.appendChild(identity);

		const side = document.createElement('span');
		side.className = 'mods-library-type';
		side.textContent = 'FABRIC';
		top.appendChild(side);

		const description = document.createElement('p');
		description.className = 'mods-library-description';
		description.textContent = project.description || 'No description provided.';

		const meta = document.createElement('div');
		meta.className = 'mods-library-meta';
		const downloads = document.createElement('span');
		downloads.textContent = new Intl.NumberFormat().format(project.downloads || 0) + ' downloads';
		const version = document.createElement('span');
		version.textContent = 'Latest ' + (project.latestVersion || 'version available');
		meta.append(downloads, version);

		const footer = document.createElement('div');
		footer.className = 'mods-library-footer';
		const details = document.createElement('span');
		details.className = 'mods-library-compat';
		details.textContent = 'Minecraft ' + state.modsLibrary.gameVersion + ' · Fabric';
		footer.appendChild(details);

		const button = document.createElement('button');
		button.type = 'button';
		button.className = project.installed ? 'btn btn--ghost btn--small' : 'btn btn--primary btn--small';
		button.textContent = project.installed ? 'Installed' : 'Install';
		button.disabled = Boolean(project.installed || state.modsLibrary.installingId || state.installing || state.performance.installing);
		if (state.modsLibrary.installingId === project.projectId) {
			button.textContent = 'Installing…';
			button.disabled = true;
		}
		button.addEventListener('click', () => installModrinthProject(project));
		footer.appendChild(button);

		card.append(top, description, meta, footer);
		container.appendChild(card);
	}
}

async function searchModrinth() {
	const query = ($('#mods-library-search')?.value || '').trim();
	const profile = currentProfile();
	const gameVersion = profile.gameVersion || state.settings.gameVersion;
	if (!gameVersion) {
		toast('Choose a Minecraft version in your profile first.', 'error');
		return;
	}

	const requestId = ++state.modsLibrary.requestId;
	state.modsLibrary.query = query;
	state.modsLibrary.gameVersion = gameVersion;
	state.modsLibrary.loading = true;
	state.modsLibrary.projects = [];
	state.modsLibrary.searched = false;
	$('#mods-library-version').textContent = 'Compatible Fabric mods for Minecraft ' + gameVersion;
	$('#mods-library-summary').textContent = query
		? 'Searching Modrinth for “' + query + '”…'
		: 'Loading popular Fabric mods for Minecraft ' + gameVersion + '…';
	renderModrinthProjects();

	try {
		const result = await api.mods.search(query, gameVersion);
		if (requestId !== state.modsLibrary.requestId) return;
		if (!result?.ok) {
			throw new Error(result?.error || 'The Modrinth library could not be reached.');
		}
		state.modsLibrary.projects = result.projects || [];
		state.modsLibrary.searched = true;
		$('#mods-library-summary').textContent = state.modsLibrary.projects.length + ' compatible mod(s) for Minecraft ' + gameVersion;
	} catch (error) {
		if (requestId !== state.modsLibrary.requestId) return;
		state.modsLibrary.projects = [];
		state.modsLibrary.searched = true;
		$('#mods-library-summary').textContent = error.message || 'Modrinth search failed.';
		toast(error.message || 'Modrinth search failed.', 'error');
	} finally {
		if (requestId === state.modsLibrary.requestId) {
			state.modsLibrary.loading = false;
			renderModrinthProjects();
		}
	}
}

async function installModrinthProject(project) {
	if (!project?.projectId || state.modsLibrary.installingId) return;
	const gameVersion = currentProfile().gameVersion || state.settings.gameVersion;
	state.modsLibrary.installingId = project.projectId;
	state.modsLibrary.progress = {
		name: project.title,
		state: 'Preparing',
		received: 0,
		total: 0
	};
	$('#mods-library-progress').hidden = false;
	$('#mods-library-progress-text').textContent = 'Preparing ' + project.title + '…';
	$('#mods-library-progress-fill').style.width = '0%';
	setStatus('Installing mod', 'busy');
	renderModrinthProjects();

	try {
		const result = await api.mods.install(project.projectId, gameVersion);
		if (!result?.ok) {
			const failed = result?.report?.failed?.[0];
			throw new Error(failed?.error || result?.error || 'The mod could not be installed.');
		}
		await loadMods();
		const installed = result.report?.installed?.length || 0;
		toast(
			installed > 0
				? project.title + ' and its required dependencies installed.'
				: project.title + ' is already up to date.',
			'success'
		);
		await searchModrinth();
	} catch (error) {
		toast(error.message || 'The mod installation failed.', 'error');
	} finally {
		state.modsLibrary.installingId = null;
		state.modsLibrary.progress = null;
		$('#mods-library-progress').hidden = true;
		$('#mods-library-progress-fill').style.width = '0%';
		setStatus('Ready');
		renderModrinthProjects();
	}
}

function handleModrinthProgress(progress) {
	if (!state.modsLibrary.installingId || !progress) return;
	state.modsLibrary.progress = progress;
	const detail = progress.detail ? ' · ' + progress.detail : '';
	$('#mods-library-progress').hidden = false;
	$('#mods-library-progress-text').textContent = (progress.name || 'Mod') + ': ' + (progress.state || 'working') + detail;
	const percent = progress.total > 0
		? Math.min(100, Math.round((progress.received / progress.total) * 100))
		: (progress.state === 'installed' || progress.state === 'skipped' ? 100 : 0);
	$('#mods-library-progress-fill').style.width = percent + '%';
}

function renderMods() {
	const table = $('#mods-table');
	table.innerHTML = '';

	const enabled = state.mods.filter((entry) => entry.enabled).length;
	const query = ($('#mods-search')?.value || '').trim().toLocaleLowerCase();
	const mode = $('#mods-filter')?.value || 'all';
	const filtered = state.mods.filter((entry) => {
		const matchesQuery = (entry.modId + ' ' + entry.name).toLocaleLowerCase().includes(query);
		const matchesMode = mode === 'all' || (mode === 'enabled' ? entry.enabled : !entry.enabled);
		return matchesQuery && matchesMode;
	});
	$('#mods-count').textContent = state.mods.length === 0
		? 'no mod'
		: `${enabled}/${state.mods.length} enabled`;
	$('#mods-tab-count').textContent = String(state.mods.length);
	const summary = $('#mods-summary');
	if (summary) summary.textContent = query || mode !== 'all'
		? `Showing ${filtered.length} of ${state.mods.length} installed mods`
		: `${state.mods.length} installed · ${enabled} enabled · ${state.mods.length - enabled} disabled`;

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

	if (filtered.length === 0) {
		table.appendChild(emptyState(
			ICON_MOD,
			'No matching mods',
			'Change the search or choose another filter.'
		));
		return;
	}

	for (const entry of filtered) {
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
	renderQuickServers();
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

		const join = rowButton('Join', 'btn--primary', () => launch(entry.address));
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
	const healthPercent = mods.length ? Math.round((installed / mods.length) * 100) : 0;
	$('#perf-health-count').textContent = mods.length ? installed + '/' + mods.length + ' installed' : 'No data';
	$('#perf-health-percent').textContent = mods.length ? healthPercent + '%' : '—';
	$('#perf-health-progress').style.width = healthPercent + '%';
	$('#perf-health-message').textContent = mods.length
		? (installed === mods.length ? 'All optimization mods are ready' : (mods.length - installed) + ' mod(s) missing')
		: (state.performance.checked ? 'No optimization pack data' : 'Waiting for local check');
	renderSetupChecklist();

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
		$('#perf-jvm-summary').textContent = settings.jvmPreset;
		$('#jvm-current').textContent = `Current preset: ${settings.jvmPreset}. It applies on the next launch.`;

		// Highlight active preset card
		$$('.preset-card').forEach((card) => {
			card.classList.toggle('is-active', card.dataset.preset === settings.jvmPreset);
		});
	}

	const preset = settings.optionsPreset;
	if (preset && preset !== 'vanilla') {
		$('#perf-options-summary').textContent = preset === 'max' ? 'Maximum FPS' : preset;
		$('#options-current').textContent = preset === 'restore'
			? 'The original options.txt is in place.'
			: `Last applied preset: ${preset}.`;
	} else {
		$('#perf-options-summary').textContent = 'Default options';
		$('#options-current').textContent = 'The original options.txt is in place.';
	}
	$('#perf-health-message').textContent = state.performance.checked ? 'Local install checked' : 'Not checked yet';
	renderSetupChecklist();
}

async function loadPerformance() {
	const result = await api.performance.list();
	if (!result.ok) {
		toast(result.error || 'The optimisation pack could not be read.', 'error');
		return;
	}

	state.performance.mods = result.mods || [];
	state.performance.state = result.state || {};
	state.performance.checked = true;
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
		await loadProfiles();
		renderDashboard();
		renderSetupChecklist();
		toast(`Added ${result.account.username}`, 'success');
	}, 'Create');

	const input = $('#offline-name');
	input.focus();
	input.addEventListener('input', () => {
		const value = input.value.trim();
		const valid = /^[A-Za-z0-9_]{3,16}$/.test(value);
		$('#offline-preview').textContent = valid
			? `Ready to play as ${value}.`
			: '3 to 16 characters, only letters, digits and underscores.';
		$('#offline-preview').style.color = valid ? 'var(--success)' : 'var(--text-muted)';
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
		await loadProfiles();
		renderDashboard();
		renderSetupChecklist();
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

async function launch(server) {
	if (state.launching) return;
	if (!state.accounts.selected) {
		toast('Add an account first.', 'error');
		return;
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
	$('#select-theme').addEventListener('change', (event) => applyTheme(event.target.value));
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

	$('#input-java').addEventListener('change', async (event) => {
		const result = await api.settings.update({ javaPath: event.target.value.trim() });
		if (!result.ok) {
			toast(result.error || 'The Java path could not be saved.', 'error');
			return;
		}
		await refreshJavaRuntime();
	});

	$('#input-accent').addEventListener('input', (event) => {
		$('#output-accent').textContent = event.target.value.toUpperCase();
		document.documentElement.style.setProperty('--accent', event.target.value);
	});
	$('#input-accent').addEventListener('change', (event) => {
		api.settings.update({ accentColor: event.target.value });
	});

	$('#input-fabric-api').addEventListener('change', (event) => {
		api.settings.update({ fabricApiVersion: event.target.value.trim() });
	});
}

async function refreshJavaRuntime() {
	const info = await api.info();
	if (!info?.ok) {
		toast(info?.error || 'Java runtime information could not be refreshed.', 'error');
		return;
	}
	state.javaRuntime = info.javaRuntime || null;
	state.settings = info.settings;
	renderDashboard();
}

function applySettings(settings) {
	state.settings = settings;
	applyTheme(settings.theme || 'dark', false);

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
	$('#select-theme').value = settings.theme === 'light' ? 'light' : 'dark';
	$('#input-accent').value = settings.accentColor;
	$('#output-accent').textContent = settings.accentColor.toUpperCase();
	$('#input-fabric-api').value = settings.fabricApiVersion;
	$('#check-g1').checked = settings.useG1gc !== false;

	document.documentElement.style.setProperty('--accent', settings.accentColor);
	paintRanges();
	renderDashboard();
	renderSetupChecklist();
}

async function loadLoaders(gameVersion) {
	const result = await api.versions.loaders(gameVersion);
	state.loaders = result.ok ? result.loaders : [];
	renderLoaders();
}

/** Creates a profile from the one the selected account plays with right now. */
async function newProfileModal() {
	const source = currentProfile();
	openModal('Create a profile', \`
		<p class="muted">Start from your current setup, then give this profile its own Minecraft version, loader and launch settings.</p>
		<label class="field">
			<span class="field-label">Profile name</span>
			<input type="text" id="profile-name" class="input" maxlength="32" placeholder="PvP, Survival, Modded…" autocomplete="off" />
		</label>
		<div class="profile-wizard-grid">
			<label class="field">
				<span class="field-label">Minecraft version</span>
				<select id="profile-version" class="input" aria-label="Minecraft version for new profile"></select>
			</label>
			<label class="field">
				<span class="field-label">Fabric loader</span>
				<select id="profile-loader" class="input" aria-label="Fabric loader for new profile"></select>
			</label>
		</div>
		<details class="profile-wizard-advanced">
			<summary>Memory and window settings</summary>
			<div class="profile-wizard-grid">
				<label class="field">
					<span class="field-label">Maximum memory (MB)</span>
					<input type="number" id="profile-memory" class="input" min="1024" max="65536" step="256" />
				</label>
				<label class="field">
					<span class="field-label">Minimum memory (MB)</span>
					<input type="number" id="profile-min-memory" class="input" min="512" max="65536" step="256" />
				</label>
				<label class="field">
					<span class="field-label">Window width</span>
					<input type="number" id="profile-width" class="input" min="640" max="7680" step="8" />
				</label>
				<label class="field">
					<span class="field-label">Window height</span>
					<input type="number" id="profile-height" class="input" min="480" max="4320" step="8" />
				</label>
			</div>
		</details>
	\`, async () => {
		const name = $('#profile-name').value.trim();
		if (!name) {
			toast('Give your profile a name first.', 'error');
			$('#profile-name').focus();
			return;
		}

		const values = {
			version: $('#profile-version').value.trim(),
			loader: $('#profile-loader').value.trim(),
			memory: Number($('#profile-memory').value),
			minMemory: Number($('#profile-min-memory').value),
			width: Number($('#profile-width').value),
			height: Number($('#profile-height').value)
		};
		if (!values.version || !values.loader) {
			toast('Choose a Minecraft version and Fabric loader.', 'error');
			return;
		}
		if (!Number.isInteger(values.memory) || !Number.isInteger(values.minMemory)
			|| values.memory < 1024 || values.memory > 65536
			|| values.minMemory < 512 || values.minMemory > values.memory) {
			toast('Check the memory values. Minimum memory cannot exceed maximum memory.', 'error');
			return;
		}
		if (!Number.isInteger(values.width) || values.width < 640 || values.width > 7680
			|| !Number.isInteger(values.height) || values.height < 480 || values.height > 4320) {
			toast('Check the window resolution values.', 'error');
			return;
		}

		const result = await api.profiles.add(name, values);
		if (!result.ok) {
			toast(result.error, 'error');
			return;
		}

		await loadProfiles();
		toast('Profile ' + name + ' created.', 'success');
	}, 'Create profile');

	const versionSelect = $('#profile-version');
	for (const option of Array.from($('#select-version').options)) {
		versionSelect.appendChild(option.cloneNode(true));
	}
	selectOption(versionSelect, source.gameVersion);

	const loaderSelect = $('#profile-loader');
	loaderSelect.disabled = true;
	const loadingLoader = document.createElement('option');
	loadingLoader.value = '';
	loadingLoader.textContent = 'Loading Fabric versions…';
	loaderSelect.appendChild(loadingLoader);
	try {
		const initialLoaders = await api.versions.loaders(source.gameVersion);
		loaderSelect.replaceChildren();
		if (initialLoaders?.ok && (initialLoaders.loaders || []).length > 0) {
			for (const loader of initialLoaders.loaders) {
				const option = document.createElement('option');
				option.value = loader;
				option.textContent = loader;
				loaderSelect.appendChild(option);
			}
			selectOption(loaderSelect, source.loaderVersion);
		} else {
			const saved = document.createElement('option');
			saved.value = source.loaderVersion || '';
			saved.textContent = source.loaderVersion || 'No Fabric loader available';
			loaderSelect.appendChild(saved);
		}
	} catch {
		loaderSelect.replaceChildren();
		const saved = document.createElement('option');
		saved.value = source.loaderVersion || '';
		saved.textContent = source.loaderVersion || 'Saved loader unavailable';
		loaderSelect.appendChild(saved);
	} finally {
		loaderSelect.disabled = false;
	}

	$('#profile-memory').value = source.maxMemory;
	$('#profile-min-memory').value = source.minMemory;
	$('#profile-width').value = source.windowWidth;
	$('#profile-height').value = source.windowHeight;

	versionSelect.addEventListener('change', async () => {
		const selectedVersion = versionSelect.value;
		loaderSelect.disabled = true;
		loaderSelect.replaceChildren();
		const loading = document.createElement('option');
		loading.value = '';
		loading.textContent = 'Loading Fabric versions…';
		loaderSelect.appendChild(loading);

		try {
			const result = await api.versions.loaders(selectedVersion);
			if (!result?.ok) throw new Error(result?.error || 'Fabric versions could not be loaded.');
			loaderSelect.replaceChildren();
			for (const loader of result.loaders || []) {
				const option = document.createElement('option');
				option.value = loader;
				option.textContent = loader;
				loaderSelect.appendChild(option);
			}
			if (loaderSelect.options.length === 0) {
				const unavailable = document.createElement('option');
				unavailable.value = '';
				unavailable.textContent = 'No Fabric loader for this version';
				loaderSelect.appendChild(unavailable);
			}
			loaderSelect.value = loaderSelect.options[0].value;
		} catch (error) {
			loaderSelect.replaceChildren();
			const unavailable = document.createElement('option');
			unavailable.value = '';
			unavailable.textContent = 'Fabric loader unavailable';
			loaderSelect.appendChild(unavailable);
			toast(error.message || 'Fabric versions could not be loaded.', 'error');
		} finally {
			loaderSelect.disabled = false;
		}
	});
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
	renderDashboard();
	renderSetupChecklist();
}

/**
 * Opens a page by its name, exactly like a click on the navigation does.
 *
 * The reports use it to bring the player to the logs instead of only telling them where the
 * files are.
 */
function showPage(name) {
	state.page = name;
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
	if (name === 'performance') loadPerformance();
	if (name === 'play') {
		setTimeout(init3DSkinViewer, 50);
	}
}

async function init() {
	initWorkspaceControls();

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
	$('#versions-search').addEventListener('input', renderVersions);
	$('#versions-filter').addEventListener('change', renderVersions);
	$('#btn-mods-refresh').addEventListener('click', loadMods);
	$('#mods-tab-installed').addEventListener('click', () => setModsTab('installed'));
	$('#mods-tab-discover').addEventListener('click', () => setModsTab('discover'));
	$('#btn-mods-library-search').addEventListener('click', searchModrinth);
	$('#mods-library-search').addEventListener('keydown', (event) => {
		if (event.key === 'Enter') searchModrinth();
	});
	$('#mods-search').addEventListener('input', renderMods);
	$('#mods-filter').addEventListener('change', renderMods);
	$('#quick-servers-open').addEventListener('click', () => showPage('servers'));
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
	api.onModrinthProgress(handleModrinthProgress);
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
	state.javaRuntime = info.javaRuntime || null;
	applyTheme(info.settings.theme || 'dark', false);
	$('#paths-output').textContent = [
		`game   ${info.paths.game}`,
		`mods   ${info.paths.mods}`,
		`assets ${info.paths.assets}`,
		`data   ${info.paths.data}`
	].join('\n');

	state.accounts = await api.accounts.get();
	renderAccounts();
	renderAccountTable();
	renderDashboard();
	renderSetupChecklist();

	await loadVersions();
	await loadLoaders(state.settings.gameVersion);
	await loadProfiles();
	await loadMods();
	await loadLogs();
	await loadServers();
	await loadHosts();
	await loadPerformance();
	renderDashboard();
	renderSetupChecklist();
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
