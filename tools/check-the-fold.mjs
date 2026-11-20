// Contracts for Aether — The Fold.
// Keep the main action simple while proving every existing destination and launcher hook survives.
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
	'btn-gate-menu', 'btn-gate-menu-close', 'gate-menu', 'gate-menu-backdrop',
	'gate-state-line', 'gate-account-summary', 'play-accounts', 'play-add-offline',
	'select-profile', 'btn-profile-new', 'btn-profile-delete', 'select-version',
	'select-loader', 'range-memory', 'output-memory', 'range-width', 'output-width',
	'range-height', 'output-height', 'console', 'install-progress', 'progress-fill',
	'progress-label', 'dock-avatar', 'dock-account-name', 'session-badge',
	'dock-version', 'dock-memory', 'btn-cancel-install', 'btn-stop', 'btn-install', 'btn-launch'
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
requireText(html, 'id="gate-menu" aria-label="Aether destinations" hidden', 'secondary navigation');
requireText(html, 'class="gate-emblem"', 'The Fold identity');
requireText(html, 'id="btn-launch" aria-label="Launch Minecraft"', 'launch action');
requireText(html, '<span>ENTER</span>', 'primary action label');
requireText(app, 'function setGateMenuOpen(open, restoreFocus = false)', 'menu controller');
requireText(app, 'function closeGateMenu()', 'menu close controller');
requireText(app, 'gateState.textContent = summary', 'live launcher status');
requireText(app, 'gateAccountSummary.textContent', 'selected player label');
requireText(css, '.gate-page', 'home screen styles');
requireText(css, '.gate-emblem', 'Fold identity styles');
requireText(css, '.gate-menu', 'secondary navigation styles');
requireText(css, '@media (max-width: 700px)', 'responsive layout');

if (failures.length) {
	for (const failure of failures) console.error('FAIL  ' + failure);
	console.error('The Fold contract: ' + failures.length + ' failure(s).');
	process.exit(1);
}

console.log('The Fold contract: passed (' + pages.size + ' destinations, ' + ids.length + ' unique element ids, ' + requiredIds.length + ' preserved launcher hooks).');
