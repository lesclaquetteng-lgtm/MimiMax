import { copyFile, mkdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const source = path.join(root, 'launcher', 'desktop', 'target', 'release', 'aether-desktop.exe');
const outputs = [
  path.join(root, 'MimiMax.exe'),
  path.join(root, 'Aether-Launcher.exe')
];

try {
  const metadata = await stat(source);
  if (!metadata.isFile() || metadata.size < 1024 * 1024) {
    throw new Error('The Tauri desktop executable is unexpectedly small or is not a file.');
  }

  for (const destination of outputs) {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
    const copied = await stat(destination);
    if (copied.size !== metadata.size) {
      throw new Error('The copied launcher size does not match the Tauri build.');
    }
    process.stdout.write('Created ' + path.relative(root, destination) + ' (' + copied.size + ' bytes)\n');
  }
} catch (error) {
  process.stderr.write('Could not create the portable Aether launcher: ' + error.message + '\n');
  process.exitCode = 1;
}
