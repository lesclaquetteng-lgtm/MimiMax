// Static contract check for the Aether Identity Studio.
import { readFile } from 'node:fs/promises';

const files = {
  html: 'launcher/gui/src/ui/index.html',
  app: 'launcher/gui/src/ui/app.js',
  css: 'launcher/gui/src/ui/styles.css',
  tokens: 'launcher/gui/src/ui/tokens.css',
  settings: 'launcher/src/settings.rs'
};

const source = Object.fromEntries(
  await Promise.all(Object.entries(files).map(async ([key, filename]) => [
    key,
    await readFile(filename, 'utf8')
  ]))
);

const failures = [];
function requireText(file, text, reason) {
  if (!source[file].includes(text)) failures.push(reason);
}
function requireExactlyOnce(file, text, reason) {
  const count = source[file].split(text).length - 1;
  if (count !== 1) failures.push(`${reason} (found ${count})`);
}
function escapeRegex(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

const ids = [
  'theme-choices',
  'surface-choices',
  'density-choices',
  'navigation-choices',
  'appearance-reset',
  'input-accent',
  'output-accent',
  'input-java',
  'range-max-memory',
  'range-min-memory',
  'check-g1',
  'input-fabric-api',
  'paths-output',
  'studio-theme-name',
  'studio-preview-density',
  'studio-preview-memory'
];

for (const id of ids) {
  requireExactlyOnce('html', `id="${id}"`, `Identity Studio control #${id} must exist exactly once`);
}

for (const [theme, accent] of Object.entries({
  graphite: '#E7E9EC',
  oxide: '#D99A69',
  field: '#C0D18A',
  blueprint: '#83C5DF'
})) {
  requireText('app', `${theme}:`, `Missing ${theme} colourway in the controller`);
  requireText('app', `${theme}: '${accent}'`, `Missing signature accent for ${theme}`);
  requireText('html', `data-theme="${theme}"`, `Missing ${theme} colourway choice`);

  for (const channel of [
    'bg', 'bg-elevated', 'surface', 'surface-hover', 'surface-active',
    'border', 'border-strong', 'accent',
    'text-primary', 'text-secondary', 'text-muted', 'text-dim'
  ]) {
    const token = `--palette-${theme}-${channel}`;
    const declaration = new RegExp(`^[\\t ]*${escapeRegex(token)}\\s*:`, 'gm');
    const count = [...source.tokens.matchAll(declaration)].length;
    if (count !== 1) failures.push(`${token} must be declared once in tokens.css (found ${count})`);
  }
}

for (const [selector, label] of [
  ['#theme-choices [data-theme]', 'colourway'],
  ['#surface-choices [data-surface]', 'surface'],
  ['#density-choices [data-density]', 'density'],
  ['#navigation-choices [data-navigation]', 'navigation']
]) {
  requireText('app', selector, `Missing ${label} interaction binding`);
}
requireText('app', 'function applyAppearance', 'Missing live appearance application');
requireText('app', 'appearanceSaveQueue', 'Appearance writes must be serialized');
requireText('app', "openModal(\n\t\t\t'Reset workspace design?'", 'Reset must use the accessible app modal');
requireText('app', 'setChoiceState(selector, attribute, selected)', 'Missing aria state synchronisation');
requireText('app', '$$(selector).forEach', 'Appearance selections must update all matching buttons');

for (const surface of ['contour', 'draft', 'plain']) {
  requireText('css', `html[data-surface="${surface}"]`, `Missing ${surface} surface rendering`);
  requireText('html', `data-surface="${surface}"`, `Missing ${surface} surface choice`);
}
requireText('css', 'html[data-density="compact"] .page', 'Compact density must affect the real page layout');
requireText('css', 'html[data-navigation="rail"] .sidebar', 'Icon rail must affect the real sidebar');
requireText('css', 'html[data-navigation="rail"] .studio-preview-frame', 'Icon rail must be reflected in the live preview');

for (const key of ['ui_density', 'surface_style', 'navigation_style']) {
  requireText('settings', `pub ${key}: String`, `Missing persisted Settings field ${key}`);
}
for (const [key, value] of [
  ['ui_density', 'roomy'],
  ['surface_style', 'contour'],
  ['navigation_style', 'wide']
]) {
  requireText('settings', `${key}: "${value}".to_string()`, `Missing default for ${key}`);
}
requireText('settings', 'appearance_settings_are_patched_and_survive_serialization', 'Missing settings round-trip regression test');
requireText('settings', 'self.theme.eq_ignore_ascii_case("dark")', 'Legacy dark theme must migrate to graphite');
requireText('settings', 'a_legacy_dark_theme_is_migrated_to_graphite_and_saved', 'Missing legacy theme migration regression test');

if (failures.length) {
  for (const failure of failures) console.error(`FAIL ${failure}`);
  console.error(`Identity Studio: ${failures.length} contract failure(s)`);
  process.exit(1);
}

console.log(`Identity Studio: ${ids.length} unique control IDs, 4 colourways × 12 palette tokens, 3 surfaces, 2 densities, 2 navigation modes — all contracts resolved.`);
