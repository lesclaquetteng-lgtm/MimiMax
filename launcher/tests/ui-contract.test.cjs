'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const html = fs.readFileSync(path.join(root, 'gui/src/ui/index.html'), 'utf8');
const app = fs.readFileSync(path.join(root, 'gui/src/ui/app.js'), 'utf8');
const css = fs.readFileSync(path.join(root, 'gui/src/ui/styles.css'), 'utf8');
const rust = fs.readFileSync(path.join(root, 'src/main.rs'), 'utf8');
const modsRust = fs.readFileSync(path.join(root, 'src/mods.rs'), 'utf8');
const bridge = fs.readFileSync(path.join(root, 'gui/src/ui/bridge.js'), 'utf8');

const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);

test('every navigation item has a matching page', () => {
	const navigation = [...html.matchAll(/class="[^"]*\bnav-item\b[^"]*"[^>]*data-page="([^"]+)"/g)]
		.map((match) => match[1]);
	const pages = new Set([...html.matchAll(/class="[^"]*\bpage\b[^"]*"[^>]*data-page="([^"]+)"/g)]
		.map((match) => match[1]));
	assert.ok(navigation.length >= 8);
	for (const name of navigation) assert.ok(pages.has(name), 'missing page: ' + name);
});

test('the redesigned interface has unique, connected UI anchors', () => {
	const required = [
		'play-accounts', 'select-profile', 'select-version', 'select-loader',
		'quick-servers', 'quick-servers-open', 'java-runtime-summary',
		'java-runtime-label', 'java-runtime-detail', 'java-runtime-settings',
		'java-runtime-status', 'versions-search', 'versions-filter', 'versions-summary',
		'mods-search', 'mods-filter', 'mods-summary', 'mods-catalog-search', 'mods-catalog-search-button', 'mods-catalog-status', 'mods-catalog-results',
		'perf-health-count', 'perf-health-progress', 'perf-health-percent',
		'perf-health-message', 'perf-jvm-summary', 'perf-options-summary',
		'command-open', 'command-palette', 'command-search-input', 'command-results',
		'btn-launch', 'btn-install', 'btn-stop'
	];
	for (const id of required) {
		assert.equal(ids.filter((candidate) => candidate === id).length, 1, '#' + id + ' must exist exactly once');
	}
});

test('the launcher supports quick-joining saved servers from the Play page', () => {
	assert.match(app, /function renderQuickServers\(\)/);
	assert.match(app, /launch\(server\.address\)/);
	assert.match(app, /renderQuickServers\(\);/);
	assert.match(html, /Quick connect/);
});

test('Java status is read from the Rust info command and can be refreshed', () => {
	assert.match(rust, /"javaRuntime": java_runtime/);
	assert.match(rust, /"requiredMajor": required_java/);
	assert.match(app, /function renderJavaRuntime\(\)/);
	assert.match(app, /async function refreshJavaRuntime\(\)/);
	assert.match(app, /info\.javaRuntime/);
});

test('versions and installed mods have searchable status/type filters', () => {
	assert.match(app, /function renderVersions\(\)/);
	assert.match(app, /version\.type === type/);
	assert.match(app, /function renderMods\(\)/);
	assert.match(app, /filter === 'enabled'/);
	assert.match(app, /filter === 'disabled'/);
});

test('the command palette is keyboard accessible and supports navigation', () => {
	assert.match(app, /function bindCommandPalette\(\)/);
	assert.match(app, /event\.key\.toLowerCase\(\) === 'k'/);
	assert.match(app, /event\.key === 'ArrowDown'/);
	assert.match(app, /event\.key === 'ArrowUp'/);
	assert.match(app, /function runCommand\(item\)/);
});

test('the Modrinth catalog calls native Rust search and install commands', () => {
	assert.match(app, /async function searchModCatalog\\(\\)/);
	assert.match(app, /async function installCatalogMod\\(project\\)/);
	assert.match(app, /api\\.mods\\.search\\(query, gameVersion\\)/);
	assert.match(app, /api\\.mods\\.install\\(project\\.projectId, gameVersion\\)/);
	assert.match(bridge, /invoke\\('mods_search'/);
	assert.match(bridge, /invoke\\('mods_install'/);
	assert.match(rust, /mods::search_catalog/);
	assert.match(rust, /mods::install_catalog_project/);
});

test('Modrinth downloads are restricted to Fabric-compatible, checksum-verified JARs', () => {
	assert.match(modsRust, /pub fn search_catalog/);
	assert.match(modsRust, /pub fn install_catalog_project/);
	assert.match(modsRust, /is_official_cdn_url/);
	assert.match(modsRust, /http::download\\(&file\\.url, &destination, Some\\(&checksum\\)/);
	assert.match(modsRust, /dependency_type\\.as_deref\\(\\) != Some\\("required"\\)/);
	assert.match(html, /third-party code loaded by Minecraft/i);
});

test('the launcher never simulates FPS in its desktop UI', () => {
	assert.doesNotMatch(app, /initFpsSplineGraph|fpsHistory|fps-spline-canvas/);
	assert.doesNotMatch(html, /fps-spline-canvas|Live FPS curve|880\+ FPS peak target/i);
	assert.match(html, /This is an installation\/configuration check, not a live FPS benchmark/);
	assert.match(app, /installed\/total/);
});

test('quick connect and command palette follow reduced-motion preferences', () => {
	assert.match(css, /\.quick-server-row/);
	assert.match(css, /\.command-dialog/);
	assert.match(css, /prefers-reduced-motion:\s*reduce/);
});
