// Contracts for CUBE Launcher — Voxel Edition.
// The historical filename stays because the check proves the shell's identity, destinations and hooks.
import { readFile } from 'node:fs/promises';

const root = 'launcher/gui/src/ui';
const [html, app, css] = await Promise.all([
	readFile(root + '/index.html', 'utf8'),
	readFile(root + '/app.js', 'utf8'),
	readFile(root + '/styles.css', 'utf8')
]);

const failures = [];
const requireText = (source, fragment, label) => {
	if (!source.includes(fragment)) failures.push(label + ': missing ' + fragment);
};

const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
if (duplicates.length) failures.push('duplicate HTML ids: ' + [...new Set(duplicates)].join(', '));

const requiredIds = [
	'voxel-scene', 'status-pill', 'status-text', 'gate-state-line',
	'stat-playtime', 'stat-worlds', 'stat-mods',
	'instance-hero', 'home-profile-name', 'home-profile-version', 'home-profile-loader',
	'home-profile-memory', 'btn-launch', 'play-account-label', 'btn-stop',
	'btn-cancel-install', 'install-progress', 'progress-fill', 'progress-label',
	'host-name', 'host-version', 'host-port', 'host-players', 'btn-host-create', 'host-list', 'home-host-list',
	'mods-search-query', 'btn-mods-search', 'mods-search-results',
	'play-accounts', 'play-add-offline', 'dock-avatar', 'dock-account-name', 'session-badge',
	'dock-version', 'dock-memory', 'select-version', 'select-loader', 'range-memory',
	'output-memory', 'range-width', 'output-width', 'range-height', 'output-height',
	'console', 'btn-install', 'range-hue', 'output-hue', 'toggle-animated-bg', 'toggle-auto-updates'
];
for (const id of requiredIds) {
	if (!ids.includes(id)) failures.push('missing launcher hook #' + id);
}

const pages = new Set(
	[...html.matchAll(/<section\b[^>]*\bdata-page="([^"]+)"/g)].map((match) => match[1])
);
for (const match of html.matchAll(/\bdata-page="([^"]+)"/g)) {
	const destination = match[1];
	if (!pages.has(destination)) failures.push('destination "' + destination + '" has no page');
}

if (html.includes('<aside class="sidebar">')) failures.push('the old permanent sidebar is still present');

// Identity: one wordmark, one rail, one voxel sky.
requireText(html, '<span class="titlebar-wordmark">CUBE LAUNCHER</span>', 'titlebar wordmark');
requireText(html, '<aside class="app-rail" aria-label="Navigation CUBE">', 'permanent rail');
requireText(html, '<span class="hero-kicker">CUBE / LAUNCHER</span>', 'hero identity');
requireText(html, 'Choisis ton monde, ajuste ton skin et plonge dans le voxel.', 'hero lede');
requireText(html, '<button type="button" class="rail-item nav-item" data-page="worlds">', 'world navigation');

// The dashboard of the design: stats, game launch, worlds, news and preferences.
requireText(html, '<div class="hero-stat">', 'dashboard stats');
requireText(html, '<span class="card-eyebrow">TON ESPACE DE JEU</span>', 'game area');
requireText(html, '<h2 id="library-title">MES MONDES</h2>', 'world library');
requireText(html, '<h2 id="news-title">ACTUALITÉS</h2>', 'news feed');
requireText(html, '<h2 id="prefs-title">RÉGLAGES</h2>', 'preferences');
requireText(html, '<button type="button" class="play-button" id="btn-launch" aria-label="Lancer Minecraft">', 'launch action');
requireText(html, '<span class="play-label">JOUER</span>', 'primary action label');
requireText(html, 'id="install-progress" hidden', 'hidden preparation bar');
requireText(html, 'id="progress-label"', 'preparation label');

// The controller keeps the dashboard alive.
requireText(app, 'function bindTilt(element)', 'instance tilt controller');
requireText(app, 'async function createHost()', 'host creation action');
requireText(app, 'async function searchMods()', 'Modrinth discovery action');
requireText(app, 'function showPage(name)', 'page navigation');
requireText(app, 'function renderDashboardStats()', 'live dashboard stats');
requireText(app, 'function setStatus(text, kind = \'\')', 'live launcher status');
requireText(app, 'gateState.textContent = summary', 'live launcher status line');
requireText(app, 'gateAccountSummary.textContent', 'selected player label');
requireText(app, 'api.onInstallProgress((progress) =>', 'preparation progress binding');

// The stylesheet renders the same dashboard.
requireText(css, '.app-rail', 'rail styles');
requireText(css, '.home-hero', 'hero styles');
requireText(css, '.instance-hero', 'active instance styles');
requireText(css, '.news-grid', 'news styles');
requireText(css, '.pref-card', 'preference card styles');
requireText(css, '.voxel-scene', 'voxel background styles');
requireText(css, 'html[data-surface="contour"]', 'contour surface styles');
requireText(css, '@media (max-width: 860px)', 'responsive layout');

if (failures.length) {
	for (const failure of failures) console.error('FAIL  ' + failure);
	console.error('The Voxel contract: ' + failures.length + ' failure(s).');
	process.exit(1);
}

console.log('Voxel contract: passed (' + pages.size + ' destinations, ' + ids.length + ' unique element ids, ' + requiredIds.length + ' preserved launcher hooks).');
