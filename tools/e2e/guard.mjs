// Refuses to drive a launcher that is not running on a throwaway home.
//
// The checks act like a player: `screens.mjs` deletes the first account and empties the mods folder,
// `profiles.mjs` deletes profiles, `failure.mjs` installs and launches. Pointed at the real profile
// that is not a test any more, it is data loss — it happened once.
//
// Two checks, because the environment is not the whole story:
//  - `requireTestHome()` reads `AETHER_HOME` and runs at import time, before the window is touched;
//  - `requireTestPaths()` reads the paths the window reports about itself (`window.aether.info()`),
//    because a window started on another home keeps writing there whatever this process believes.
//
// Both only accept a home under `tools/`, where the e2e homes, the theme home and the UI homes live.
// `AETHER_E2E_ALLOW_ANY_HOME=1` lifts them, for a disposable machine and never for a player.
import { realpathSync } from 'node:fs';
import { isAbsolute, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The `tools/` folder of the repository, the only place a test home may live. */
const TOOLS = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** The real path of a folder: a junction inside tools/ must not lead to a real profile. */
function real(path) {
	try {
		return realpathSync.native(path);
	} catch {
		// The folder may not exist yet, it is created by the caller.
		return path;
	}
}

/** Whether a path lives under `tools/`, case and junction included. */
function insideTools(path) {
	const difference = relative(real(TOOLS), real(resolve(path)));
	// `D:\home` against `C:\…\tools` is absolute, `tools-evil` starts with `..`: both are outside.
	return difference === '' || (!difference.startsWith('..') && !isAbsolute(difference));
}

const overridden = () => process.env.AETHER_E2E_ALLOW_ANY_HOME === '1';

/** Stops the process when `AETHER_HOME` is missing or points at a real profile. */
export function requireTestHome() {
	const home = (process.env.AETHER_HOME || '').trim();

	if (!home) {
		throw new Error(
			'AETHER_HOME is not set: these checks delete accounts, mods and profiles, so they refuse to ' +
				'drive the real profile. Start the launcher on a home under tools/ (tools/e2e/home), then run again.',
		);
	}

	const absolute = resolve(home);

	if (!insideTools(absolute) && !overridden()) {
		throw new Error(
			`AETHER_HOME points outside tools/ (${absolute}): these checks delete accounts, mods and ` +
				'profiles, so they only run on a throwaway home. Set AETHER_E2E_ALLOW_ANY_HOME=1 to override ' +
				'on a disposable machine.',
		);
	}

	return absolute;
}

/**
 * Stops the process when the window itself reports a home outside `tools/`.
 *
 * Call it with the answer of `window.aether.info()` before deleting anything: it is the only way to
 * know where the launcher that is actually running writes.
 */
export function requireTestPaths(info) {
	const home = String(info?.paths?.home || '').trim();

	if (!home) {
		throw new Error('the window did not report its home: refusing to touch data it cannot name');
	}

	if (!insideTools(home) && !overridden()) {
		throw new Error(
			`the launcher is running on ${home}, outside tools/: that is a real profile, and these checks ` +
				'delete accounts, mods and profiles. Stop it and start it on a home under tools/.',
		);
	}

	return home;
}
