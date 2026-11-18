import fs from 'node:fs';

const root = 'C:/Users/ee/Desktop/MimiMax/launcher/gui/src/ui';
const app = fs.readFileSync(`${root}/app.js`, 'utf8');
const html = fs.readFileSync(`${root}/index.html`, 'utf8');

const refs = [...app.matchAll(/\$\('#([A-Za-z0-9_-]+)'\)/g)].map((m) => m[1]);
const ids = [...html.matchAll(/id="([A-Za-z0-9_-]+)"/g)].map((m) => m[1]);
const missing = [...new Set(refs.filter((ref) => !ids.includes(ref)))];

console.log(`refs=${new Set(refs).size} ids=${ids.length} missing=${JSON.stringify(missing)}`);
