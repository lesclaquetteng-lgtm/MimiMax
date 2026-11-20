'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const html = fs.readFileSync(path.join(root, 'gui/src/ui/index.html'), 'utf8');
const app = fs.readFileSync(path.join(root, 'gui/src/ui/app.js'), 'utf8');
const styles = fs.readFileSync(path.join(root, 'gui/src/ui/styles.css'), 'utf8');
const bridge = fs.readFileSync(path.join(root, 'gui/src/ui/bridge.js'), 'utf8');
const preload = fs.readFileSync(path.join(root, 'gui/src/preload.js'), 'utf8');
const rustMods = fs.readFileSync(path.join(root, 'src/modrinth.rs'), 'utf8');

test('every navigation entry points to an existing page', () => {
	const navigation = [...html.matchAll(/class="[^"]*\bnav-item\b[^"]*"[^>]*data-page="([^"]+)"/g)]
		.map((match) => match[1]);
	const pages = new Set([...html.matchAll(/class="[^"]*\bpage\b[^"]*"[^>]*data-page="([^"]+)"/g)]
		.map((match) => match[1]));
	assert.ok(navigation.length >= 8, 'expected all launcher sections in navigation');
	for (const name of navigation) assert.ok(pages.has(name), 'missing page for ' + name);
});

test('the workspace components have unique DOM anchors', () => {
	const ids = [
		'sidebar-toggle', 'command-open', 'command-palette', 'command-search', 'command-results',
		'theme-toggle', 'select-theme', 'hero-version', 'dashboard-account', 'dashboard-account-meta',
		'dashboard-profile', 'dashboard-profile-meta', 'dashboard-java', 'dashboard-java-meta',
		'dashboard-status', 'setup-card', 'setup-dismiss', 'setup-account', 'setup-account-state',
		'setup-profile', 'setup-profile-state', 'setup-performance', 'setup-performance-state',
		'perf-health-count', 'perf-health-progress', 'perf-health-percent', 'perf-jvm-summary',
		'perf-options-summary', 'perf-health-message', 'quick-servers', 'quick-servers-open', 'java-runtime-status',
		'mods-search', 'mods-filter', 'mods-summary', 'versions-search', 'versions-filter', 'versions-summary',
		'mods-tab-installed', 'mods-tab-discover', 'mods-tab-count', 'mods-installed-panel', 'mods-discover-panel',
		'mods-library-search', 'btn-mods-library-search', 'mods-library-version', 'mods-library-summary',
		'mods-library-results', 'mods-library-progress', 'mods-library-progress-text', 'mods-library-progress-fill'
	];
	for (const id of ids) {
		const occurrences = [...html.matchAll(new RegExp('\\bid="' + id + '"', 'g'))].length;
		assert.equal(occurrences, 1, '#' + id + ' should exist exactly once');
	}
});

test('the account, profile and launch entry points remain present', () => {
	for (const id of ['btn-add-offline', 'btn-add-microsoft', 'select-profile',
		'btn-profile-new', 'btn-profile-delete', 'btn-install', 'btn-launch', 'btn-stop']) {
		assert.match(html, new RegExp('\\bid="' + id + '"'));
	}
	assert.match(html, /data-page="accounts"/);
	assert.match(html, /data-page="play"/);
});

test('the command palette and keyboard shortcuts are wired', () => {
	assert.match(app, /function openCommandPalette\(\)/);
	assert.match(app, /function renderCommandResults\(\)/);
	assert.match(app, /event\.key\.toLowerCase\(\) === 'k'/);
	assert.match(app, /\['play', 'accounts', 'versions', 'mods', 'servers', 'performance', 'logs', 'settings'\]/);
});

test('appearance is persisted through the Rust settings API', () => {
	assert.match(app, /api\.settings\.update\(\{ theme: normalized \}\)/);
	assert.match(html, /id="select-theme"/);
	assert.match(app, /document\.documentElement\.dataset\.theme/);
	assert.match(styles, /html\[data-theme="light"\]/);
});


test('the dashboard provides saved-server quick play and real Java diagnostics', () => {
	assert.match(html, /id="quick-servers"/);
	assert.match(app, /function renderQuickServers\(\)/);
	assert.match(app, /launch\(server\.address\)/);
	assert.match(app, /info\.javaRuntime/);
	assert.match(app, /runtime\?\.compatible/);
});

test('the installed mod library and version list can be searched and filtered', () => {
	for (const id of ['mods-search', 'mods-filter', 'versions-search', 'versions-filter']) {
		assert.match(html, new RegExp('\\\\bid="' + id + '"'));
	}
	assert.match(app, /function renderMods\(\)/);
	assert.match(app, /function renderVersions\(\)/);
	assert.match(app, /const matchesType = type === 'all' \|\| version\.type === type/);
});

test('the Modrinth library is wired through the Rust bridge in both desktop shells', () => {
	assert.match(html, /id="mods-tab-discover"/);
	assert.match(html, /id="mods-library-results"/);
	assert.match(html, /cdn\.modrinth\.com/);
	assert.match(app, /function searchModrinth\(\)/);
	assert.match(app, /function installModrinthProject\(project\)/);
	assert.match(app, /function handleModrinthProgress\(progress\)/);
	assert.match(bridge, /invoke\('mods_search'/);
	assert.match(bridge, /invoke\('mods_install'/);
	assert.match(bridge, /listen\('mods:progress'/);
	assert.match(preload, /ipcRenderer\.invoke\('mods:search'/);
	assert.match(preload, /ipcRenderer\.invoke\('mods:install'/);
});

test('the Rust mod browser restricts installs to checksummed Modrinth JARs', () => {
	assert.match(rustMods, /fn trusted_download_url/);
	assert.match(rustMods, /cdn\.modrinth\.com/);
	assert.match(rustMods, /valid_sha1/);
	assert.match(rustMods, /client\.download\(&file\.url, &destination, Some\(expected_sha1\)/);
	assert.match(rustMods, /dependency_type == "required"/);
	assert.match(rustMods, /not managed by Aether/);
});

test('performance overview does not publish simulated FPS telemetry', () => {
	assert.doesNotMatch(app, /fpsHistory|initFpsSplineGraph/);
	assert.match(app, /installed \/ mods\.length/);
	assert.match(html, /No FPS is simulated here/);
});
