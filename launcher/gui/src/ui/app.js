const api = window.aether;

const state = {
	page: 'play',
	accounts: { accounts: [], selected: null },
	settings: {},
	versions: [],
	loaders: [],
	mods: [],
	servers: { servers: [], pings: {} },
	hosts: [],
	performance: { mods: [], state: {}, installing: false },
	info: null,
	sessionLines: 0,
	installing: false,
	launching: false
};

// The state of the interface, read by the end to end check to see the flags go back to idle.
window.aetherState = state;

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => Array.from(document.querySelectorAll(selector));

/* ------------------------------------------------------- local preferences */

const PLAY_TIME_KEY = 'cube.playtimeSeconds';
const HUE_KEY = 'cube.brandHue';
const MOTION_KEY = 'cube.animatedBackground';
const UPDATES_KEY = 'cube.autoUpdates';
const DEFAULT_HUE = 96;

function readLocal(key, fallback) {
	try {
		const value = window.localStorage.getItem(key);
		return value === null ? fallback : value;
	} catch {
		return fallback;
	}
}

function writeLocal(key, value) {
	try {
		window.localStorage.setItem(key, String(value));
	} catch {
		// A locked profile only loses a cosmetic preference, never the launcher.
	}
}

/** `6 Go` reads better than `6144 MB` in the dashboard; small values stay in MB. */
function formatMemory(megabytes) {
	const amount = Number(megabytes) || 0;
	if (amount >= 1024 && amount % 1024 === 0) return `${amount / 1024} Go`;
	if (amount > 1024) return `${(amount / 1024).toFixed(1)} Go`;
	return `${amount} MB`;
}

function readPlaytime() {
	return Math.max(0, Number(readLocal(PLAY_TIME_KEY, 0)) || 0);
}

function formatPlaytime(seconds) {
	const total = Math.max(0, Math.floor(seconds));
	const hours = Math.floor(total / 3600);
	const minutes = Math.floor((total % 3600) / 60);
	if (hours > 0) return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
	return `${minutes}m`;
}

function applyHue(hue) {
	const value = Math.min(360, Math.max(0, Math.round(Number(hue) || DEFAULT_HUE)));
	document.documentElement.style.setProperty('--brand-hue', String(value));
	const output = $('#output-hue');
	if (output) output.textContent = `${value}°`;
	const input = $('#range-hue');
	if (input && Number(input.value) !== value) input.value = String(value);
}

function applyMotion(enabled) {
	document.documentElement.dataset.motion = enabled ? 'on' : 'off';
	const toggle = $('#toggle-animated-bg');
	if (toggle) {
		toggle.setAttribute('aria-checked', String(enabled));
	}
	if (!enabled) {
		const hero = $('#instance-hero');
		if (hero) hero.style.transform = '';
	}
}

function applyAutoUpdates(enabled) {
	const toggle = $('#toggle-auto-updates');
	if (toggle) toggle.setAttribute('aria-checked', String(enabled));
}

function loadPreferences() {
	applyHue(readLocal(HUE_KEY, DEFAULT_HUE));
	applyMotion(readLocal(MOTION_KEY, 'on') !== 'off');
	applyAutoUpdates(readLocal(UPDATES_KEY, 'on') !== 'off');
}

function renderDashboardStats() {
	const playtime = $('#stat-playtime');
	if (playtime) playtime.textContent = formatPlaytime(readPlaytime());

	const worlds = $('#stat-worlds');
	if (worlds) worlds.textContent = String((state.hosts || []).length);
	const worldCount = $('#host-count-home');
	if (worldCount) worldCount.textContent = `${(state.hosts || []).length} ${(state.hosts || []).length === 1 ? 'monde' : 'mondes'}`;

	const mods = $('#stat-mods');
	if (mods) mods.textContent = String((state.mods || []).length);
}

/** A launch adds its elapsed seconds to the local counter shown in the hero. */
function recordPlaytime(startedAt) {
	const elapsed = Math.round((Date.now() - startedAt) / 1000);
	if (!Number.isFinite(elapsed) || elapsed <= 0) return;

	writeLocal(PLAY_TIME_KEY, readPlaytime() + elapsed);
	renderDashboardStats();
}

/** The pointer tilts the active instance card on its two axes. */
function bindTilt(element) {
	if (!element) return;

	const reset = () => {
		element.style.transform = '';
	};

	element.addEventListener('pointermove', (event) => {
		if (event.pointerType && event.pointerType !== 'mouse') return;
		if (document.documentElement.dataset.motion === 'off') {
			reset();
			return;
		}

		const box = element.getBoundingClientRect();
		if (!box.width || !box.height) return;

		const x = (event.clientX - box.left) / box.width - 0.5;
		const y = (event.clientY - box.top) / box.height - 0.5;
		element.style.transform = `perspective(1200px) rotateX(${(-y * 4).toFixed(2)}deg) rotateY(${(x * 5).toFixed(2)}deg)`;
	});

	element.addEventListener('pointerleave', reset);
	element.addEventListener('pointercancel', reset);
}

/** The full stories behind the three teaser cards of the home page. */
const NEWS_STORIES = Object.freeze({
	snapshot: {
		tag: 'SNAPSHOT',
		title: 'Biome des cimes',
		lede: 'Les sommets enneigés arrivent avec leurs reliefs escarpés, leurs pins givrés et une faune qui ne craint pas le froid.',
		body: [
			'Les nouvelles montagnes se dressent au-dessus des forêts et cachent des vallées entières. Les ruisseaux gelés y dessinent des chemins naturels, tandis que les grottes de glace abritent des minerais rares.',
			'Garde un œil sur les créatures des hauteurs : certaines descendent à la nuit tombée. Prépare une pioche, des torches et un bon manteau de laine.'
		]
	},
	update: {
		tag: 'UPDATE',
		title: 'Refonte de la redstone',
		lede: 'Les circuits gagnent en lisibilité et en puissance : nouveaux composants, signaux plus stables et de quoi imaginer des machines plus ambitieuses.',
		body: [
			'Les répéteurs et les comparateurs affichent désormais leur état sans ambiguïté, et les pistons réagissent plus vite aux impulsions courtes.',
			'Les constructions existantes continuent de fonctionner. Un passage par le menu Performance permet de vérifier que tes fermes automatiques tournent toujours au rythme attendu.'
		]
	},
	event: {
		tag: 'ÉVÉNEMENT',
		title: "Festival d'automne",
		lede: "Jusqu'au 30 novembre, la communauté se retrouve sur les serveurs du launcher pour bâtir, explorer et partager ses plus belles trouvailles.",
		body: [
			'Des défis quotidiens récompensent les constructions automnales : lanternes, champs de citrouilles et cabanes en bois. Les participants cumulent des points pour débloquer un skin exclusif.',
			'Les serveurs de l’événement sont accessibles depuis la page Serveurs. Aucune installation supplémentaire n’est nécessaire.'
		]
	}
});

function openNews(kind) {
	const story = NEWS_STORIES[kind];
	if (!story) return;

	const paragraphs = [story.lede, ...story.body]
		.map((text) => `<p>${text}</p>`)
		.join('');
	openModal(story.title, `<span class="news-tag">${story.tag}</span>${paragraphs}`, null, 'Fermer');
}

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
	if (pill) pill.className = `gate-status${kind ? ` is-${kind}` : ''}`;
	const label = $('#status-text');
	if (label) label.textContent = text;

	const gateState = $('#gate-state-line');
	if (gateState) {
		const message = String(text).toLowerCase();
		let summary = 'PRÊT À JOUER';
		if (kind === 'error' || message.includes('erreur') || message.includes('planter')) {
			summary = 'UNE ERREUR À CORRIGER';
		} else if (message.includes('en jeu')) {
			summary = 'LE JEU TOURNE';
		} else if (kind === 'busy' && (message.includes('lanc') || message.includes('démarrage') || message.includes('connexion') || message.includes('arrêt') || message.includes('rejoin'))) {
			summary = 'LANCEMENT EN COURS';
		} else if (kind === 'busy' || message.includes('install') || message.includes('prépar') || message.includes('télécharg')) {
			summary = 'PRÉPARATION DU MONDE';
		}
		gateState.textContent = summary;
	}
	if (pill) pill.setAttribute('data-state', kind || 'ready');

	const gameState = $('#home-game-state');
	if (gameState) gameState.textContent = text;
}

/**
 * The rail marks the current destination with its own `::before` bar, so the indicator element is
 * only kept for the older markup and hidden when it is present.
 */
function positionDockIndicator() {
	const indicator = $('#dock-indicator');
	if (indicator) indicator.style.display = 'none';
}

/** Parses Minecraft MOTD formatting codes (§0-f, §l,m,n,o,r) into styled sanitized HTML spans. */
function parseMinecraftMotd(raw) {
	if (!raw) return '<span class="muted">Aucune description fournie</span>';
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
	return html || '<span class="muted">Un serveur Minecraft</span>';
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
		setStatus('Erreur', 'error');
		toast(error?.message || "L'action n'a pas pu aboutir.", 'error');
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

// The pure helpers live in format.js so that `node --test` can check them without a window.
function formatBytes(bytes) {
	return window.AetherFormat.formatBytes(bytes);
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
			'Aucun compte',
			'Ajoute un compte hors ligne pour jouer tout de suite, un pseudo suffit.'
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
			tag.textContent = account.type === 'offline' ? 'Hors ligne' : 'Microsoft';

			row.append(avatar, meta, tag);
			row.addEventListener('click', async () => {
				state.accounts = await api.accounts.select(account.id);
				renderAccounts();
				renderAccountTable();
				applyProfile(state.settings);
			});
			list.appendChild(row);
		}
	}

	const selected = state.accounts.accounts.find((a) => a.id === state.accounts.selected);
	$('#session-badge').textContent = selected
		? (selected.type === 'offline' ? 'session hors ligne' : 'session Microsoft')
		: 'aucun compte sélectionné';
	const gateAccountSummary = $('#gate-account-summary');
	if (gateAccountSummary) {
		gateAccountSummary.textContent = selected
			? `${selected.username} / ${selected.type === 'offline' ? 'HORS LIGNE' : 'MICROSOFT'}`
			: 'AUCUN COMPTE SÉLECTIONNÉ';
	}
	const playerAction = $('.gate-player-change span');
	if (playerAction) playerAction.textContent = selected ? 'CHANGER' : 'AJOUTER UN JOUEUR';
	const launchButton = $('#btn-launch');
	if (launchButton) {
		launchButton.disabled = !selected;
		launchButton.setAttribute('aria-disabled', String(!selected));
	}

	renderDock();
}

function renderAccountTable() {
	const table = $('#account-table');
	table.innerHTML = '';

	if (state.accounts.accounts.length === 0) {
		const empty = emptyState(
			ICON_USER,
			'Aucun compte pour le moment',
			'Les comptes hors ligne sont créés sur cette machine. Les comptes Microsoft passent par la connexion par code.',
			'Ajouter un compte hors ligne'
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
		tag.textContent = account.type === 'offline' ? 'Hors ligne' : 'Microsoft';

		const select = document.createElement('button');
		select.className = `btn btn--ghost${account.id === state.accounts.selected ? ' is-connected' : ''}`;
		select.textContent = account.id === state.accounts.selected ? 'Connecté' : 'Choisir';
		select.setAttribute('aria-pressed', String(account.id === state.accounts.selected));
		select.addEventListener('click', async () => {
			state.accounts = await api.accounts.select(account.id);
			renderAccounts();
			renderAccountTable();
			applyProfile(state.settings);
		});

		const remove = document.createElement('button');
		remove.className = 'icon-btn';
		remove.innerHTML = '<svg viewBox="0 0 24 24"><path d="M4 7h16M9 7V5h6v2M6 7l1 13h10l1-13" /></svg>';
		remove.title = 'Retirer';
		remove.addEventListener('click', async () => {
			state.accounts = await api.accounts.remove(account.id);
			renderAccounts();
			renderAccountTable();
			toast(`Compte ${account.username} retiré.`, 'success');
		});

		row.append(avatar, meta, tag, select, remove);
		table.appendChild(row);
	}
}

function renderVersions() {
	const table = $('#version-table');
	if (!table) return;
	table.innerHTML = '';

	if (state.versions.length === 0) {
		table.appendChild(emptyState(
			ICON_STACK,
			'Lecture des versions…',
			'Les versions sont publiées par Mojang, la liste se remplit dès que la réponse arrive.'
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
		action.textContent = 'Utiliser';
		action.addEventListener('click', async () => {
			await updateProfile({ version: version.id });
			toast(`Minecraft ${version.id} sélectionné.`, 'success');
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
		option.textContent = wanted || 'par défaut';
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
		state.sessionLines === 1 ? '1 ligne' : `${state.sessionLines} lignes`;
	$('#session-log-badge').classList.add('is-live');
}

/** A new game starts a new session, the previous output is dropped. */
function clearSession() {
	for (const element of consoles()) {
		element.textContent = '';
	}

	state.sessionLines = 0;
	$('#session-log-badge').textContent = 'vide';
	$('#session-log-badge').classList.remove('is-live');
}

/* -------------------------------------------------------------------- mods */

function renderMods() {
	const table = $('#mods-table');
	table.innerHTML = '';

	const enabled = state.mods.filter((entry) => entry.enabled).length;
	$('#mods-count').textContent = state.mods.length === 0
		? 'aucun mod'
		: `${enabled}/${state.mods.length} actifs`;

	if (state.mods.length === 0) {
		const empty = emptyState(
			ICON_MOD,
			'Aucun mod dans le dossier',
			'Glisse un .jar dans le dossier des mods, ou lance « Installer / réparer » pour télécharger le client Aether.',
			'Ouvrir le dossier'
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
		sub.textContent = `${formatBytes(entry.size)} · ${entry.enabled ? 'actif' : 'désactivé'}`;
		meta.append(name, sub);

		const toggle = document.createElement('button');
		toggle.className = 'switch';
		toggle.setAttribute('role', 'switch');
		toggle.setAttribute('aria-checked', String(entry.enabled));
		toggle.setAttribute('aria-label', `${entry.enabled ? 'Désactiver' : 'Activer'} ${entry.modId}`);
		toggle.addEventListener('click', async () => {
			toggle.disabled = true;
			const result = await api.mods.toggle(entry.name, !entry.enabled);
			toggle.disabled = false;

			if (!result.ok) {
				toast(result.error || "Le mod n'a pas pu être modifié.", 'error');
				return;
			}

			state.mods = result.mods;
			renderMods();
			renderDashboardStats();
			toast(`${entry.modId} ${entry.enabled ? 'désactivé' : 'activé'}`, 'success');
		});

		row.append(icon, meta, toggle);
		table.appendChild(row);
	}
	renderDashboardStats();
}

async function loadMods() {
	const result = await api.mods.list();
	if (!result.ok) {
		toast(result.error || "Les mods n'ont pas pu être lus.", 'error');
		return;
	}

	state.mods = result.mods || [];
	$('#mods-path').textContent = result.folder || '';
	renderMods();
	renderDashboardStats();
}

async function openModsFolder() {
	const result = await api.mods.openFolder();
	if (!result.ok) toast(result.error || "Le dossier n'a pas pu être ouvert.", 'error');
}

async function searchMods() {
	const query = $('#mods-search-query').value.trim();
	const results = $('#mods-search-results');
	if (query.length < 2) { toast('Entre au moins deux caractères.', 'error'); return; }
	results.innerHTML = '<div class="mod-search-empty">Recherche sur Modrinth…</div>';
	const result = await api.mods.search(query, state.settings.gameVersion || '1.21.11');
	if (!result.ok) { results.innerHTML = '<div class="mod-search-empty">Recherche impossible. Réessaie dans un instant.</div>'; toast(result.error, 'error'); return; }
	results.replaceChildren();
	for (const mod of result.mods || []) {
		const card = document.createElement('article'); card.className = 'mod-result-card';
		const heading = document.createElement('div'); heading.className = 'mod-result-heading';
		const title = document.createElement('h3'); title.textContent = mod.title;
		const compatible = document.createElement('span'); compatible.className = `badge${mod.compatible ? ' is-live' : ''}`; compatible.textContent = mod.compatible ? 'Fabric compatible' : 'Incompatible';
		heading.append(title, compatible);
		const description = document.createElement('p'); description.textContent = mod.description || 'Mod communautaire sur Modrinth.';
		const footer = document.createElement('div'); footer.className = 'mod-result-footer';
		const downloads = document.createElement('span'); downloads.className = 'muted'; downloads.textContent = `${formatCompactNumber(mod.downloads)} téléchargements`;
		const install = document.createElement('button'); install.type = 'button'; install.className = 'btn btn--primary btn--small'; install.textContent = 'Installer'; install.disabled = !mod.compatible;
		install.addEventListener('click', async () => {
			install.disabled = true; install.textContent = 'Vérification…';
			const installed = await api.mods.install(mod.slug, state.settings.gameVersion || '1.21.11');
			if (!installed.ok) { install.disabled = false; install.textContent = 'Réessayer'; toast(installed.error || 'Installation refusée.', 'error'); return; }
			state.mods = installed.mods || state.mods; renderMods(); renderDashboardStats(); install.textContent = 'Installé'; toast(`${mod.title} installé et vérifié.`, 'success');
		});
		footer.append(downloads, install); card.append(heading, description, footer); results.appendChild(card);
	}
	if (!result.mods?.length) results.innerHTML = '<div class="mod-search-empty">Aucun mod trouvé pour cette version de Minecraft.</div>';
}

function formatCompactNumber(value) {
	return new Intl.NumberFormat('fr-FR', { notation: 'compact', maximumFractionDigits: 1 }).format(Number(value) || 0);
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
	if (!ping.online) return ping.error ? `hors ligne · ${ping.error}` : 'hors ligne';

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
	$('#server-count').textContent = list.length === 1 ? '1 serveur' : `${list.length} serveurs`;

	if (list.length === 0) {
		table.appendChild(emptyState(
			ICON_SERVER,
			'Aucun serveur enregistré',
			'Ajoute une adresse ci-dessus, ou héberge un serveur local qui accepte n\'importe quel pseudo.'
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
			offBadge.textContent = 'Hors ligne';
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

		const join = rowButton('Rejoindre', 'btn--ghost', () => launch(entry.address));
		const test = rowButton('Tester', 'btn--ghost', async () => {
			test.disabled = true;
			await testAddress(entry.address);
			test.disabled = false;
		});
		const remove = rowButton('Retirer', 'btn--danger', async () => {
			remove.disabled = true;
			const result = await api.servers.remove(entry.address);
			if (!result.ok) {
				remove.disabled = false;
				toast(result.error || "Le serveur n'a pas pu être retiré.", 'error');
				return;
			}

			state.servers.servers = result.servers || [];
			delete state.servers.pings[entry.address];
			renderServers();
			toast(`${entry.name} retiré.`, 'success');
		});

		actions.append(join, test, remove);

		card.append(head, motdView, actions);
		table.appendChild(card);
	}
}

async function loadServers() {
	const result = await api.servers.list();
	if (!result.ok) {
		toast(result.error || "La liste des serveurs n'a pas pu être lue.", 'error');
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
		toast('Saisis une adresse à tester.', 'error');
		return;
	}

	const verdict = $('#server-verdict');
	verdict.textContent = `Test de ${target}…`;

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
			parts.push(`Comptes hors ligne acceptés (le serveur a répondu avec ${result.packet}).`);
		} else if (probe.verdict === 'premium') {
			parts.push(`Premium uniquement : ${result.detail}.`);
		} else if (probe.verdict === 'refused') {
			parts.push(`Refusé : ${result.detail}.`);
		} else {
			parts.push(`Injoignable : ${result.detail}.`);
		}
	} else {
		parts.push(probe.error || 'Le test de connexion a échoué.');
	}

	if (ping.ok) {
		const info = ping.ping;
		parts.push(info.online
			? `${info.playersOnline}/${info.playersMax} joueurs · ${info.latencyMs} ms · ${info.versionName}`
			: `hors ligne : ${info.error}`);
	}

	verdict.textContent = parts.join(' ');
	const accepted = probe.ok && probe.verdict === 'offline';
	// The verdict stays on the page: a toast would cover the rows while the player reads it.
	if (accepted) toast(`${target} accepte un compte hors ligne.`, 'success');
}

async function addServer() {
	const address = $('#server-address').value.trim();
	if (!address) {
		toast('Saisis l\'adresse d\'un serveur.', 'error');
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
		toast(result.error || "Le serveur n'a pas pu être ajouté.", 'error');
		return;
	}

	state.servers.servers = result.servers || [];
	$('#server-name').value = '';
	$('#server-address').value = '';
	renderServers();
	toast(`${name} ajouté.`, 'success');
	pingServer(address);
}

/* --------------------------------------------------------------- local servers */

function renderHosts() {
	const table = $('#host-list');
	table.innerHTML = '';

	const list = state.hosts || [];
	$('#host-count').textContent = `${list.length} ${list.length === 1 ? 'monde' : 'mondes'}`;
	const countBadge = $('#host-count-home');
	if (countBadge) countBadge.textContent = `${list.length} ${list.length === 1 ? 'monde' : 'mondes'}`;

	if (list.length === 0) {
		table.appendChild(emptyState(
			ICON_SERVER,
			'Aucun serveur local',
			'Crée ton premier monde depuis le formulaire ci-dessus. Le launcher prépare Fabric et le démarre.'
		));
		renderWorldList($('#home-host-list'), list);
		renderDashboardStats();
		updateHomeHostStatus();
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
		sub.textContent = `${address} · Minecraft ${server.gameVersion} · ${server.onlineMode ? 'compte Microsoft requis' : 'pseudo libre'}`;
		meta.append(name, sub);

		const actions = document.createElement('div');
		actions.className = 'row-actions';
		const status = rowBadge(server.running ? 'EN LIGNE' : 'HORS LIGNE', server.running);
		actions.appendChild(status);

		if (server.running) {
			const invite = document.createElement('button');
			invite.type = 'button';
			invite.className = 'btn btn--ghost btn--small';
			invite.textContent = 'Copier l’adresse';
			invite.addEventListener('click', async () => {
				try { await navigator.clipboard.writeText(address); toast(`Adresse ${address} copiée.`, 'success'); }
				catch { toast(`Adresse à partager : ${address}`, 'success'); }
			});
			actions.appendChild(invite);
			actions.appendChild(rowButton('Rejoindre', 'btn--primary', () => launch(address)));
			actions.appendChild(rowButton('Arrêter', 'btn--danger', () => stopHost(server.name)));
		} else {
			actions.appendChild(rowButton('Démarrer', 'btn--primary', () => startHost(server.name)));
		}

		row.append(icon, meta, actions);
		row.classList.add('home-world-card');
		table.appendChild(row);
	}
	renderWorldList($('#home-host-list'), list);
	$('#host-count-home').textContent = `${list.length} ${list.length === 1 ? 'monde' : 'mondes'}`;
	renderDashboardStats();
	updateHomeHostStatus();
}

function renderWorldList(container, worlds) {
	if (!container) return;
	container.replaceChildren();
	if (!worlds.length) {
		container.appendChild(emptyState(ICON_SERVER, 'Pas encore de monde', 'Crée ton premier serveur et invite tes amis sur le même réseau.', 'Créer un monde'));
		container.querySelector('[data-empty-action]')?.addEventListener('click', () => showPage('worlds'));
		return;
	}
	for (const world of worlds.slice(0, 3)) {
		const card = document.createElement('article');
		card.className = 'home-world-card';
		const icon = document.createElement('span'); icon.className = 'mod-icon'; icon.innerHTML = ICON_SERVER;
		const meta = document.createElement('div'); meta.className = 'mod-meta';
		const title = document.createElement('strong'); title.className = 'mod-name'; title.textContent = world.name;
		const details = document.createElement('span'); details.className = 'mod-sub'; details.textContent = `${world.gameVersion} · ${world.running ? `En ligne · 127.0.0.1:${world.port}` : 'Arrêté'}`;
		meta.append(title, details);
		const action = rowButton(world.running ? 'Ouvrir' : 'Démarrer', world.running ? 'btn--primary' : 'btn--ghost', () => world.running ? launch(`127.0.0.1:${world.port}`) : startHost(world.name));
		card.append(icon, meta, action); container.appendChild(card);
	}
}

async function loadHosts() {
	const result = await api.hosts.list();
	if (!result.ok) {
		toast(result.error || "Les serveurs locaux n'ont pas pu être lus.", 'error');
		return;
	}

	state.hosts = result.servers || [];
	renderHosts();
}

async function createHost() {
	const name = $('#host-name').value.trim();
	if (!name) {
		toast('Donne un nom au monde.', 'error');
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
			maxPlayers: Number($('#host-players').value) || 8,
			gameVersion: $('#host-version').value || state.settings.gameVersion
		})
	);

	if (!result) return;
	if (!result.ok) {
		toast(result.error || "Le serveur n'a pas pu être créé.", 'error');
		return;
	}

	state.hosts = result.servers || [];
	$('#host-name').value = '';
	renderHosts();
	toast(`${name} est prêt. Démarrage du monde…`, 'success');
	await startHost(name);
}

function updateHomeHostStatus() {
	const running = (state.hosts || []).find((host) => host.running);
	const label = $('#home-host-status');
	if (label) label.textContent = running ? `${running.name} est ouvert · 127.0.0.1:${running.port}` : 'Ton jeu est prêt quand tu veux.';
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
	setStatus('Démarrage du serveur…', 'busy');
	const result = await withBusy(() => setStatus('Prêt'), () => api.hosts.start(name));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || "Le serveur n'a pas pu être démarré.", 'error');
		return;
	}

	await loadHosts();
	toast(`${name} démarre…`, 'success');

	const address = `127.0.0.1:${result.server.port}`;
	const ready = await waitForServer(address);

	if (ready) {
		renderHosts();
		toast(`${name} est prêt sur ${address}.`, 'success');
	} else {
		toast(`${name} démarre encore, surveille console.log dans son dossier.`, 'error');
	}
}

async function stopHost(name) {
	setStatus('Arrêt du serveur…', 'busy');
	const result = await withBusy(() => setStatus('Prêt'), () => api.hosts.stop(name));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || "Le serveur n'a pas pu être arrêté.", 'error');
		return;
	}

	state.hosts = result.servers || state.hosts;
	renderHosts();
	toast(`${name} arrêté et le monde a été sauvegardé.`, 'success');
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
			'Le pack est illisible',
			'Rafraîchis la liste, le cœur lit le dossier des mods du jeu.'
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

		row.append(icon, meta, rowBadge(mod.installed ? 'installé' : 'manquant', mod.installed));
		table.appendChild(row);
	}
}

function renderPerformanceState() {
	const settings = state.performance.state || {};

	if (settings.jvmPreset) {
		selectOption($('#select-jvm'), settings.jvmPreset);
		$('#jvm-current').textContent = `Preset actuel : ${settings.jvmPreset}. Il s'applique au prochain lancement.`;

		// Highlight active preset card
		$$('.preset-card').forEach((card) => {
			card.classList.toggle('is-active', card.dataset.preset === settings.jvmPreset);
		});
	}

	const preset = settings.optionsPreset;
	if (preset && preset !== 'vanilla') {
		$('#options-current').textContent = preset === 'restore'
			? 'Le options.txt d\'origine est en place.'
			: `Dernier preset appliqué : ${preset}.`;
	} else {
		$('#options-current').textContent = 'Le options.txt d\'origine est en place.';
	}
}

async function loadPerformance() {
	const result = await api.performance.list();
	if (!result.ok) {
		toast(result.error || "Le pack d'optimisation n'a pas pu être lu.", 'error');
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
	$('#perf-path').textContent = 'Recherche du pack sur Modrinth…';

	const result = await withBusy(
		() => {
			state.performance.installing = false;
			button.disabled = false;
		},
		() => api.performance.install([])
	);

	if (!result) return;
	if (!result.mods) {
		toast(result.error || "Le pack n'a pas pu être installé.", 'error');
		return;
	}

	state.performance.mods = result.mods;
	state.performance.state = result.state || state.performance.state;
	renderPerformance();
	renderPerformanceState();

	const failed = (result.report?.failed || []).length;
	if (failed > 0) {
		toast(`${failed} mod(s) n'ont pas pu être installés.`, 'error');
	} else {
		toast("Le pack d'optimisation est installé. Redémarre le jeu.", 'success');
	}
}

async function tuneOptions(preset) {
	const result = await withBusy(() => {}, () => api.performance.tune(preset));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || "Les options n'ont pas pu être écrites.", 'error');
		return;
	}

	state.performance.state = result.state || state.performance.state;
	renderPerformanceState();

	if (preset === 'restore') {
		toast('Les options d\'origine sont revenues.', 'success');
		return;
	}

	const changes = result.report?.changes?.length || 0;
	toast(`${preset === 'max' ? 'FPS maximum' : 'Équilibré'} : ${changes} option(s) modifiée(s).`, 'success');
}

async function applyJvm() {
	const preset = $('#select-jvm').value;
	const result = await withBusy(() => {}, () => api.performance.jvm(preset));

	if (!result) return;
	if (!result.ok) {
		toast(result.error || "Le preset JVM n'a pas pu être enregistré.", 'error');
		return;
	}

	state.performance.state = result.state || state.performance.state;
	renderPerformanceState();
	toast(`Preset JVM réglé sur ${preset}, il s'applique au prochain lancement.`, 'success');
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
		? `Les ${result.keep} dernières sessions sont conservées, les fichiers plus anciens sont supprimés automatiquement.`
		: '';
}

/** A list of sessions that could not be read says why instead of looking empty. */
function renderSessionsError(error) {
	const table = $('#sessions-table');
	table.innerHTML = '';
	$('#sessions-count').textContent = 'illisible';
	table.appendChild(emptyState(
		ICON_LOG,
		'Les sessions n\'ont pas pu être lues',
		error || "Le cœur n'a pas répondu."
	));
}

function renderSessions(sessions) {
	const table = $('#sessions-table');
	table.innerHTML = '';

	$('#sessions-count').textContent = sessions.length === 0
		? 'aucune session'
		: sessions.length === 1 ? '1 session' : `${sessions.length} sessions`;

	if (sessions.length === 0) {
		table.appendChild(emptyState(
			ICON_LOG,
			'Aucune session',
			'Chaque lancement écrit la sortie du jeu dans aether/logs/session-<date>.log.'
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
		view.textContent = 'Voir';
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
			'La session n\'a pas pu être lue',
			result.error || "Le cœur n'a pas répondu."
		));
		return;
	}

	const tail = result.session;
	const label = document.createElement('p');
	label.className = 'log-path';
	label.textContent = `${tail.total} lignes · ${tail.path}`;

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
			'Le journal n\'a pas pu être lu',
			result.error || "Le cœur n'a pas répondu."
		));
		return;
	}

	const tail = result.install;
	$('#install-log-path').textContent = tail.exists
		? `${tail.total} lignes · ${tail.path}`
		: tail.path;

	if (!tail.exists || tail.lines.length === 0) {
		body.appendChild(emptyState(
			ICON_LOG,
			'Aucune installation journalisée',
			'Chaque téléchargement écrit une ligne ici. Lance « Installer / réparer » pour le remplir.'
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

function openModal(title, bodyHtml, onConfirm, confirmLabel = 'Confirmer') {
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
	openModal('Ajouter un compte hors ligne', `
		<p class="muted">Un compte hors ligne n'a besoin que d'un pseudo. L'identifiant est généré exactement
		comme Minecraft le fait, le joueur garde donc la même identité sur tous les serveurs hors ligne.</p>
		<label class="field">
			<span class="field-label">Pseudo du joueur</span>
			<input type="text" id="offline-name" class="input" maxlength="16" placeholder="Notch" />
		</label>
		<p class="muted" id="offline-preview">De 3 à 16 caractères, lettres, chiffres et tirets bas uniquement.</p>
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
		toast(`Compte ${result.account.username} ajouté.`, 'success');
	}, 'Créer');

	const input = $('#offline-name');
	const preview = $('#offline-preview');
	input.focus();

	// Every keystroke restarts the wait: Mojang is asked once the name has settled, not sixteen
	// times. The answer of an older name is dropped when a newer one is already on its way.
	let waiting = 0;
	let asked = 0;

	const describe = (value, valid) => {
		if (!valid) {
			preview.textContent = 'De 3 à 16 caractères, lettres, chiffres et tirets bas uniquement.';
			preview.style.color = 'var(--text-muted)';
			return;
		}

		preview.textContent = `Prêt à jouer en tant que ${value}.`;
		preview.style.color = 'var(--color-success-text)';

		const ticket = ++asked;
		api.accounts.checkName(value).then((status) => {
			if (ticket !== asked || !status || !status.ok || !status.premium) return;

			preview.textContent =
				`${status.name} appartient à un compte Mojang : les serveurs craqués comme JartexNetwork ` +
				`refusent ce pseudo ("Unauthorized: You cannot join the server"). Choisis un pseudo que ` +
				`personne ne possède, par exemple ${value}9281.`;
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
		preview.textContent = `Prêt à jouer en tant que ${value}.`;
		preview.style.color = 'var(--color-success-text)';
		waiting = setTimeout(() => describe(value, true), 450);
	});
}

async function microsoftAccountModal() {
	// The core prints the code first and finishes the sign in in the background.
	const pending = api.accounts.microsoft.signIn();

	openModal('Se connecter avec Microsoft', `
		<p class="muted">Ouvre la page suivante et saisis le code pour te connecter avec ton
		compte Minecraft.</p>
		<div class="code-box">
			<div class="code-label">Ton code</div>
			<div class="code-value" id="device-code">…</div>
		</div>
		<p class="muted" style="margin-top:10px"><a id="verify-link" target="_blank"
			rel="noreferrer">En attente du code…</a></p>
	`, async () => {
		setStatus('Connexion…', 'busy');
		const result = await pending;
		setStatus('Prêt');
		if (!result.ok) {
			toast(result.error || 'La connexion a échoué.', 'error');
			return;
		}
		state.accounts = await api.accounts.get();
		renderAccounts();
		renderAccountTable();
		toast(`Connecté en tant que ${result.account.username}.`, 'success');
	}, 'Je me suis connecté');
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

async function performInstall() {
	if (state.installing) return;
	state.installing = true;
	setStatus('Installation…', 'busy');

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
		$('#install-wizard').hidden = true;
		setStatus('Prêt');
		toast("L'installation a été annulée.", 'success');
		return;
	}

	if (!result.ok) {
		$('#install-wizard').hidden = true;
		setStatus('Erreur', 'error');
		toast(result.error, 'error');
		loadLogs();
		await showInstallReport(result.error);
		return;
	}
	setStatus('Prêt');
	$('#install-wizard').hidden = true;
	toast('Le jeu est prêt à être lancé.', 'success');
	loadLogs();
}

/* Installation Studio: a focused, guided shell around the existing installer. */
let installWizardStep = 1;
function wizardSetStep(step) {
	installWizardStep = step;
	$$('.wizard-page').forEach((page) => { page.hidden = Number(page.dataset.wizardPage) !== step; });
	$$('.install-step').forEach((item) => { const n = Number(item.dataset.step); item.classList.toggle('is-active', n === step); item.classList.toggle('is-done', n < step); });
	$('#install-wizard-back').hidden = step === 1 || step === 3;
	$('#install-wizard-next').hidden = step === 3;
	$('#install-wizard-cancel').hidden = step !== 3;
	$('#wizard-status').textContent = step === 3 ? 'DÉPLOIEMENT / EN DIRECT' : `ÉTAPE 0${step} / 02`;
}
function openInstallWizard() {
	const version = $('#wizard-version'); const loader = $('#wizard-loader');
	version.innerHTML = $('#select-version').innerHTML; loader.innerHTML = $('#select-loader').innerHTML;
	version.value = $('#select-version').value; loader.value = $('#select-loader').value;
	$('#install-wizard').hidden = false; wizardSetStep(1);
}
function closeInstallWizard() { if (!state.installing) $('#install-wizard').hidden = true; }
async function wizardNext() {
	if (installWizardStep === 1) { $('#select-version').value = $('#wizard-version').value; $('#select-loader').value = $('#wizard-loader').value; $('#wizard-review-version').textContent = $('#wizard-version').selectedOptions[0]?.textContent || $('#wizard-version').value; $('#wizard-review-loader').textContent = $('#wizard-loader').selectedOptions[0]?.textContent || $('#wizard-loader').value; wizardSetStep(2); return; }
	if (installWizardStep === 2) { $('#install-wizard-next').hidden = true; wizardSetStep(3); await performInstall(); }
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
	lines = lines.map(redactSupportLine);

	// The shell hands the code over as an unsigned value, so a killed game arrived as 4294967295
	// while its own log said -1. `| 0` reads it back as the signed 32-bit code Windows reports.
	const code = exitCode === null || exitCode === undefined ? 0 : Number(exitCode) | 0;

	const settings = state.settings;
	const text = [
		'CUBE Launcher — rapport',
		'',
		`problème    ${problem}`,
		code ? `code sortie ${code}` : null,
		`version     ${settings.gameVersion}`,
		`loader      ${settings.loaderVersion}`,
		`java        ${settings.javaPath ? 'chemin personnalisé' : 'détecté automatiquement'}`,
		`session     ${session ? session.name : 'aucun fichier de session'}`,
		'',
		'dernières lignes',
		...(lines.length ? lines : ['(le journal de session est vide)'])
	]
		.filter((line) => line !== null)
		.join('\n');

	openModal(title, `
		<p class="muted">Le launcher a rédigé ce rapport à partir de la dernière session. Envoie-le
		quand tu demandes de l'aide.</p>
		<pre class="console console--tall" id="crash-report"></pre>
		<p><button class="btn btn--ghost" id="report-open-logs" type="button">Ouvrir les journaux</button></p>
	`, async () => {
		const copied = await api.copy(text);
		toast(
			copied && copied.ok ? 'Le rapport a été copié.' : "Le rapport n'a pas pu être copié.",
			copied && copied.ok ? 'success' : 'error'
		);
	}, 'Copier le rapport');

	$('#crash-report').textContent = text;

	// The report is a summary, the whole session is one click away.
	$('#report-open-logs').addEventListener('click', () => {
		closeModal();
		showPage('logs');
		if (session) openSession(session);
	});
}

/** Remove account identifiers, local paths and common credential formats from copyable reports. */
function redactSupportLine(value) {
	let line = String(value ?? '');
	const privateValues = (state.accounts?.accounts || []).flatMap((account) => [account.username, account.uuid]);
	for (const privateValue of privateValues) {
		if (privateValue && privateValue.length >= 3) {
			line = line.replace(new RegExp(privateValue.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi'), '[compte]');
		}
	}
	return line
		.replace(/(Bearer\s+)[A-Za-z0-9._~+/-]+=*/gi, '$1[masqué]')
		.replace(/((?:access|refresh)[_-]?token["'=:\s]+)[^\s,;]+/gi, '$1[masqué]')
		.replace(/[A-Z]:\\Users\\[^\\\s]+/gi, '[chemin utilisateur]')
		.replace(/\/home\/[^/\s]+/g, '[chemin utilisateur]');
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
	const lines = (tail && tail.exists ? tail.lines : []).map(redactSupportLine);

	const text = [
		'CUBE Launcher — rapport d\'installation',
		'',
		`problème    ${redactSupportLine(problem)}`,
		`version     ${$('#select-version').value}`,
		`loader      ${$('#select-loader').value}`,
		`journal     ${tail ? 'install.log' : 'indisponible'}`,
		'',
		'dernières lignes',
	...(lines.length ? lines : ['(le journal d\'installation est vide)'])
	].join('\n');

	openModal("L'installation a échoué", `
		<p class="muted">Le launcher a rédigé ce rapport à partir du journal d'installation. Envoie-le
		quand tu demandes de l'aide. Les fichiers déjà téléchargés sont conservés, la prochaine
		installation reprend là où elle s'est arrêtée.</p>
		<pre class="console console--tall" id="install-report"></pre>
		<p><button class="btn btn--ghost" id="report-open-logs" type="button">Ouvrir les journaux</button></p>
	`, async () => {
		const copied = await api.copy(text);
		toast(
			copied && copied.ok ? 'Le rapport a été copié.' : "Le rapport n'a pas pu être copié.",
			copied && copied.ok ? 'success' : 'error'
		);
	}, 'Copier le rapport');

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

	openModal('Ce pseudo appartient à un compte Mojang', `
		<p><strong>${status.name}</strong> est enregistré par un compte Mojang. Les serveurs craqués
		protègent ces pseudos : ils refusent un client craqué qui en présente un, et le jeu affiche
		seulement <em>« Unauthorized: You cannot join the server »</em>.</p>
		<p class="muted">Crée un compte hors ligne avec un pseudo que personne ne possède — par
		exemple <strong>${status.name}9281</strong> — puis rejoins ${server} à nouveau. Le solo et
		les serveurs que tu héberges acceptent n'importe quel pseudo.</p>
		<p><button class="btn btn--ghost btn--small" id="premium-continue">Rejoindre quand même</button></p>
	`, () => offlineAccountModal(), 'Choisir un autre pseudo');

	$('#premium-continue').onclick = () => {
		closeModal();
		launch(server, true);
	};

	return false;
}

async function launch(server, skipNameCheck = false) {
	if (state.launching) return;
	if (!state.accounts.selected) {
		toast("Ajoute d'abord un compte.", 'error');
		return;
	}

	if (server && !skipNameCheck && !isLocalServer(server)) {
		const accepted = await offlineNameIsAccepted(server);
		if (!accepted) return;
	}

	state.launching = true;
	const startedAt = Date.now();
	setStatus(server ? `Connexion à ${server}…` : 'Lancement…', 'busy');
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
	recordPlaytime(startedAt);

	if (result.stopped) {
		setStatus('Prêt');
		toast('Le jeu a été arrêté.', 'success');
		return;
	}

	if (!result.ok) {
		setStatus('Erreur', 'error');
		toast(result.error, 'error');

		// A core that died without an answer is a crash, a request it refused is not.
		if (result.crashed) {
			loadLogs();
			await showCrashReport({ title: 'Le cœur du launcher s\'est arrêté', problem: result.error });
		}
		return;
	}

	setStatus('Prêt');
	// Same signed reading as the report: the shell sends a killed game back as 4294967295.
	const code = result.exitCode ? Number(result.exitCode) | 0 : 0;
	if (code) {
		toast(`Le jeu s'est arrêté avec le code ${code}.`, 'error');
		loadLogs();
		await showCrashReport({
			title: 'Le jeu s\'est arrêté',
			problem: `le jeu s'est arrêté avec le code ${code}`,
			exitCode: code
		});
		return;
	}

	toast('Le jeu s\'est fermé.', 'success');
}

/** Asks the core to stop the installation that is running. */
async function cancelInstall() {
	$('#btn-cancel-install').disabled = true;

	const result = await withBusy(() => {}, () => api.installStop());

	if (!result) return;
	if (!result.ok) {
		$('#btn-cancel-install').disabled = false;
		toast(result.error || "Aucune installation n'est en cours.", 'error');
		return;
	}

	// The answer of the installation arrives on its own and clears the view.
	if (state.installing) setStatus('Annulation…', 'busy');
}

async function stopGame() {
	const result = await withBusy(() => {}, () => api.stop());
	if (!result) return;
	if (!result.ok) {
		toast(result.error || "Le jeu n'est pas en cours d'exécution.", 'error');
		return;
	}

	setStatus('Arrêt…', 'busy');
}

/* -------------------------------------------------------------------- init */

/** The filled part of a slider follows its value. */
function paintRange(input) {
	const percent = window.AetherFormat.rangePercent(input.value, input.min, input.max);
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
		$('#output-max-memory').textContent = formatMemory(maxMemory.value);
	});
	minMemory.addEventListener('input', () => {
		$('#output-min-memory').textContent = formatMemory(minMemory.value);
	});
	// The memory controls write the active profile, like the resolution controls below.
	maxMemory.addEventListener('change', () => {
		updateProfile({ memory: Number(maxMemory.value) });
	});
	minMemory.addEventListener('change', () => {
		updateProfile({ minMemory: Number(minMemory.value) });
	});

	// The Performance page writes the same active profile, from its own slider.
	const performanceMemory = $('#range-perf-memory');
	if (performanceMemory) {
		performanceMemory.addEventListener('input', () => {
			$('#output-perf-memory').textContent = formatMemory(performanceMemory.value);
		});
		performanceMemory.addEventListener('change', () => {
			updateProfile({ memory: Number(performanceMemory.value) });
		});
	}

	// The settings page resolution controls write the active profile, like the Home controls.
	const settingsWidth = $('#range-width-settings');
	const settingsHeight = $('#range-height-settings');
	if (settingsWidth) {
		settingsWidth.addEventListener('input', () => {
			$('#output-width-settings').textContent = settingsWidth.value;
		});
		settingsWidth.addEventListener('change', () => {
			updateProfile({ width: Number(settingsWidth.value) });
		});
	}
	if (settingsHeight) {
		settingsHeight.addEventListener('input', () => {
			$('#output-height-settings').textContent = settingsHeight.value;
		});
		settingsHeight.addEventListener('change', () => {
			updateProfile({ height: Number(settingsHeight.value) });
		});
	}

	$('#range-memory').addEventListener('input', (event) => {
		$('#output-memory').textContent = formatMemory(event.target.value);
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
	$('#appearance-reset').addEventListener('click', () => {
		openModal(
			'Réinitialiser le design de l\'espace ?',
			'<p>La matière, le signal personnel, la surface, l\'espacement et la navigation reviendront à leurs valeurs par défaut.</p>',
			async () => {
				if (await saveAppearance({ ...APPEARANCE_DEFAULTS })) {
					toast('Design de l\'espace restauré.', 'success');
				}
			},
			'Réinitialiser'
		);
	});

	$('#input-fabric-api').addEventListener('change', (event) => {
		api.settings.update({ fabricApiVersion: event.target.value.trim() });
	});
}

function applySettings(settings) {
	state.settings = settings;

	$('#range-memory').value = settings.maxMemory;
	$('#output-memory').textContent = formatMemory(settings.maxMemory);
	$('#range-max-memory').value = settings.maxMemory;
	$('#output-max-memory').textContent = formatMemory(settings.maxMemory);
	$('#range-min-memory').value = settings.minMemory;
	$('#output-min-memory').textContent = formatMemory(settings.minMemory);
	$('#range-width').value = settings.windowWidth;
	$('#output-width').textContent = settings.windowWidth;
	$('#range-height').value = settings.windowHeight;
	$('#output-height').textContent = settings.windowHeight;
	$('#range-perf-memory').value = settings.maxMemory;
	$('#output-perf-memory').textContent = formatMemory(settings.maxMemory);
	const settingsWidth = $('#range-width-settings');
	const settingsHeight = $('#range-height-settings');
	if (settingsWidth) {
		settingsWidth.value = settings.windowWidth;
		$('#output-width-settings').textContent = settings.windowWidth;
	}
	if (settingsHeight) {
		settingsHeight.value = settings.windowHeight;
		$('#output-height-settings').textContent = settings.windowHeight;
	}
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

function applyAccent(color, usePaletteDefault = false) {
	const value = String(color ?? '').trim();
	const root = document.documentElement.style;

	themeCache = null;

	if (usePaletteDefault || !value) {
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

const THEME_SIGNATURE_ACCENTS = Object.freeze({
	graphite: '#E7E9EC',
	oxide: '#D99A69',
	field: '#C0D18A',
	blueprint: '#83C5DF'
});

const PALETTE_CHANNELS = Object.freeze([
	'bg', 'bg-elevated', 'surface', 'surface-hover', 'surface-active',
	'border', 'border-strong', 'accent',
	'text-primary', 'text-secondary', 'text-muted', 'text-dim'
]);

function setChoiceState(selector, attribute, selected) {
	$$(selector).forEach((button) => {
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

	// One alias per design token keeps the central token audit strict while switching materials.
	PALETTE_CHANNELS.forEach((channel) => {
		// Build the CSS function name in pieces so the source token scanner does not mistake
		// a runtime-selected palette alias for an incomplete CSS custom-property reference.
		const paletteReference = 'v' + 'ar(' + '--palette-' + theme + '-' + channel + ')';
		root.style.setProperty(`--palette-${channel}`, paletteReference);
	});

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
		const storedAccent = String(settings.accentColor || '').trim();
		const hasCustomAccent = /^#[0-9a-f]{6}$/i.test(storedAccent)
			&& !LEGACY_ACCENT_DEFAULTS.has(storedAccent.toLowerCase());
		const accent = hasCustomAccent ? storedAccent : THEME_SIGNATURE_ACCENTS[theme];
		$('#input-accent').value = accent;
		$('#output-accent').textContent = accent.toUpperCase();
		applyAccent(accent, !hasCustomAccent);
	}
}

/** Saves appearance changes sequentially so fast clicks cannot overwrite each other's settings. */
let appearanceSaveQueue = Promise.resolve();

function saveAppearance(patch) {
	const commit = async () => {
		const previous = { ...state.settings };
		const optimistic = { ...previous, ...patch };
		applyAppearance(optimistic);

		try {
			const result = await api.settings.update(patch);
			if (!result || !result.ok) {
				state.settings = previous;
				applyAppearance(previous);
				toast(result?.error || "L'apparence n'a pas pu être enregistrée.", 'error');
				return false;
			}
			state.settings = { ...optimistic, ...(result.settings || {}) };
			applyAppearance(state.settings);
			return true;
		} catch (error) {
			state.settings = previous;
			applyAppearance(previous);
			toast(`L'apparence n'a pas pu être enregistrée : ${error.message}`, 'error');
			return false;
		}
	};

	const operation = appearanceSaveQueue.then(commit, commit);
	appearanceSaveQueue = operation.then(() => undefined, () => undefined);
	return operation;
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

/** The profile the selected account plays with: the values the play controls show. */
function currentProfile() {
	return {
		name: 'Minecraft',
		gameVersion: state.settings.gameVersion,
		loaderVersion: state.settings.loaderVersion,
		maxMemory: state.settings.maxMemory,
		minMemory: state.settings.minMemory,
		windowWidth: state.settings.windowWidth,
		windowHeight: state.settings.windowHeight
	};
}

/** Writes the runtime facts of the profile the selected account plays with. */
function renderSystemInfo() {
	const info = state.info;
	if (!info) return;

	const profile = currentProfile();
	const threads = navigator.hardwareConcurrency;
	$('#system-info').textContent = [
		`Launcher    CUBE ${info.version}`,
		`Plateforme  ${navigator.platform || navigator.userAgentData?.platform || 'inconnue'}${threads ? ` · ${threads} threads CPU` : ''}`,
		`Java        ${info.settings.javaPath || 'détecté automatiquement'}`,
		`Mémoire     ${profile.maxMemory} MB (initial ${profile.minMemory} MB)`,
		`Fenêtre     ${profile.windowWidth} × ${profile.windowHeight}`,
		`Données     ${info.paths.game}`
	].join('\n');
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
	if (name) name.textContent = account ? account.username : 'Aucun compte';

	const version = $('#dock-version');
	if (version) {
		version.textContent = profile.loaderVersion
			? `${profile.gameVersion} · Fabric ${profile.loaderVersion}`
			: profile.gameVersion;
	}

	const memory = $('#dock-memory');
	if (memory) memory.textContent = formatMemory(profile.maxMemory);

	const playAccount = $('#play-account-label');
	if (playAccount) {
		playAccount.textContent = account
			? `CONNECTÉ EN TANT QUE ${account.username.toUpperCase()}`
			: 'AUCUN COMPTE SÉLECTIONNÉ';
	}

	const profileName = $('#home-profile-name');
	if (profileName) profileName.textContent = profile.name || 'Default';
	const profileVersion = $('#home-profile-version');
	if (profileVersion) profileVersion.textContent = profile.gameVersion || '—';
	const profileLoader = $('#home-profile-loader');
	if (profileLoader) profileLoader.textContent = profile.loaderVersion ? `Fabric ${profile.loaderVersion}` : 'Vanilla';
	const profileMemory = $('#home-profile-memory');
	if (profileMemory) profileMemory.textContent = formatMemory(profile.maxMemory);
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
	$('#output-memory').textContent = formatMemory(profile.maxMemory);
	$('#range-width').value = profile.windowWidth;
	$('#output-width').textContent = profile.windowWidth;
	$('#range-height').value = profile.windowHeight;
	$('#output-height').textContent = profile.windowHeight;

	// The Settings and Performance sliders read the same allocation as the Home control.
	const maxMemory = $('#range-max-memory');
	if (maxMemory) {
		maxMemory.value = profile.maxMemory;
		$('#output-max-memory').textContent = formatMemory(profile.maxMemory);
	}
	const minMemory = $('#range-min-memory');
	if (minMemory) {
		minMemory.value = profile.minMemory;
		$('#output-min-memory').textContent = formatMemory(profile.minMemory);
	}
	const performanceMemory = $('#range-perf-memory');
	if (performanceMemory) {
		performanceMemory.value = profile.maxMemory;
		$('#output-perf-memory').textContent = formatMemory(profile.maxMemory);
	}

	// The Settings page shows the same profile values, from its own controls.
	const settingsWidth = $('#range-width-settings');
	const settingsHeight = $('#range-height-settings');
	if (settingsWidth) {
		settingsWidth.value = profile.windowWidth;
		$('#output-width-settings').textContent = profile.windowWidth;
	}
	if (settingsHeight) {
		settingsHeight.value = profile.windowHeight;
		$('#output-height-settings').textContent = profile.windowHeight;
	}

	selectOption($('#select-version'), profile.gameVersion);
	selectOption($('#select-loader'), profile.loaderVersion);

	paintRanges();
	renderDock();
	renderSystemInfo();
}

/** Writes game controls into the launcher-wide settings shared by every account. */
async function updateProfile(patch) {
	const settingsPatch = {};
	if (patch.version !== undefined) settingsPatch.gameVersion = patch.version;
	if (patch.loader !== undefined) settingsPatch.loaderVersion = patch.loader;
	if (patch.memory !== undefined) settingsPatch.maxMemory = patch.memory;
	if (patch.minMemory !== undefined) settingsPatch.minMemory = patch.minMemory;
	if (patch.width !== undefined) settingsPatch.windowWidth = patch.width;
	if (patch.height !== undefined) settingsPatch.windowHeight = patch.height;
	const result = await api.settings.update(settingsPatch);
	if (!result?.ok) return toast(result?.error || 'Réglage non enregistré.', 'error');
	state.settings = { ...state.settings, ...(result.settings || settingsPatch) };
	applyProfile(state.settings);
}

/**
 * Opens a page by its name, exactly like a click on the navigation does.
 *
 * The reports use it to bring the player to the logs instead of only telling them where the
 * files are.
 */
function showPage(name) {
	const target = $(`.page[data-page="${name}"]`);
	if (!target) return;

	state.page = name;
	document.body.dataset.page = name;
	$$('.nav-item').forEach((other) => {
		const match = other.dataset.page === name;
		other.classList.toggle('is-active', match);
		if (match) other.setAttribute('aria-current', 'page');
		else other.removeAttribute('aria-current');
	});

	$$('.page').forEach((page) => page.classList.remove('is-active'));
	target.classList.add('is-active');

	// The indicator slides to the destination once the new page has its box.
	requestAnimationFrame(positionDockIndicator);

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
	}
}


async function init() {
	// Window controls.
	$$('[data-window]').forEach((button) => {
		button.addEventListener('click', () => api.window(button.dataset.window));
	});

	// Cosmetic preferences live in the interface, not in the core settings.
	loadPreferences();

	// The rail and the inline "manage" links share the same navigation contract.
	$$('.nav-item').forEach((item) => {
		item.addEventListener('click', () => showPage(item.dataset.page));
	});

	// The hue repaints the whole launcher, the two switches are local preferences.
	const hue = $('#range-hue');
	if (hue) {
		hue.addEventListener('input', () => {
			applyHue(hue.value);
			writeLocal(HUE_KEY, Math.round(Number(hue.value)));
		});
	}

	const animatedBackground = $('#toggle-animated-bg');
	if (animatedBackground) {
		animatedBackground.addEventListener('click', () => {
			const next = animatedBackground.getAttribute('aria-checked') !== 'true';
			applyMotion(next);
			writeLocal(MOTION_KEY, next ? 'on' : 'off');
		});
	}

	const autoUpdates = $('#toggle-auto-updates');
	if (autoUpdates) {
		autoUpdates.addEventListener('click', () => {
			const next = autoUpdates.getAttribute('aria-checked') !== 'true';
			applyAutoUpdates(next);
			writeLocal(UPDATES_KEY, next ? 'on' : 'off');
		});
	}

	// "Lire la suite" opens the matching story from the news feed.
	$$('[data-news]').forEach((button) => {
		button.addEventListener('click', () => openNews(button.dataset.news));
	});

	// The active instance card tilts under the pointer, unless motion is disabled.
	bindTilt($('#instance-hero'));

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
	$('#btn-install').addEventListener('click', openInstallWizard);
	$('#install-wizard-next').addEventListener('click', wizardNext);
	$('#install-wizard-back').addEventListener('click', () => wizardSetStep(Math.max(1, installWizardStep - 1)));
	$('#install-wizard-close').addEventListener('click', closeInstallWizard);
	$('#install-wizard-cancel').addEventListener('click', cancelInstall);
	$('#btn-cancel-install').addEventListener('click', cancelInstall);
	$('#btn-launch').addEventListener('click', () => launch());
	$('#btn-stop').addEventListener('click', stopGame);
	$('#btn-refresh-versions').addEventListener('click', loadVersions);
	$('#btn-mods-refresh').addEventListener('click', loadMods);
	$('#btn-mods-folder').addEventListener('click', openModsFolder);
	$('#btn-mods-search').addEventListener('click', searchMods);
	$('#mods-search-query').addEventListener('keydown', (event) => { if (event.key === 'Enter') searchMods(); });
	$('#btn-logs-refresh').addEventListener('click', loadLogs);

	// The servers and the performance pages.
	$('#btn-server-add').addEventListener('click', addServer);
	$('#btn-server-probe').addEventListener('click', () => testAddress());
	$('#btn-servers-refresh')?.addEventListener('click', loadServers);
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
		positionDockIndicator();
		clearTimeout(resizeDebounce);
		resizeDebounce = setTimeout(() => {
			const activePage = state.page;
			showPage(activePage);
		}, 150);
	});

	// The bundled display face changes the box of the dock once it arrives.
	document.fonts?.ready.then(positionDockIndicator).catch(() => {});

	// The game output.
	api.onGameOutput((line) => {
		log(line);
		// The first line of the game means that Java started, the launch only answers at its end.
		if (state.launching) setStatus('En jeu', 'busy');
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
		$('#wizard-task').textContent = progress.task;
		$('#wizard-percent').textContent = `${percent}%`;
		$('#wizard-progress-fill').style.width = `${percent}%`;
		$('#wizard-progress-detail').textContent = percent >= 100 ? 'Étape terminée' : 'Téléchargement et vérification des ressources';
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
		setStatus('Erreur', 'error');
		return;
	}

	state.info = info;
	applySettings(info.settings);
	renderSystemInfo();
	$('#paths-output').textContent = [
		`jeu    ${info.paths.game}`,
		`mods   ${info.paths.mods}`,
		`assets ${info.paths.assets}`,
		`données ${info.paths.data}`
	].join('\n');

	// Real values only: the launcher build, the Java runtime it will use, the allocation and the
	// thread count the WebView reports. applyProfile refreshes them once the active profile is known.
	positionDockIndicator();

	state.accounts = await api.accounts.get();
	renderAccounts();
	renderAccountTable();

	await loadVersions();
	const hostVersion = $('#host-version');
	if (hostVersion) { hostVersion.innerHTML = $('#select-version').innerHTML; hostVersion.value = state.settings.gameVersion; }
	const modVersion = $('#mods-search-version');
	if (modVersion) modVersion.textContent = `Fabric · Minecraft ${state.settings.gameVersion}`;
	await loadLoaders(state.settings.gameVersion);
	applyProfile(state.settings);
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
	toast(`Le launcher n'a pas pu démarrer : ${error.message}`, 'error');
	setStatus('Erreur', 'error');
});
