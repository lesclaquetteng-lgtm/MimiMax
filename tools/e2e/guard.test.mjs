// Proves the guard that keeps the checks away from a real profile.
//
// Run with: node --test tools/e2e/guard.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { requireTestHome, requireTestPaths } from './guard.mjs';

const TOOLS = fileURLToPath(new URL('..', import.meta.url));

/** Sets AETHER_HOME for one test and returns the way back. */
function withHome(home) {
	const previous = process.env.AETHER_HOME;

	if (home === undefined) delete process.env.AETHER_HOME;
	else process.env.AETHER_HOME = home;

	return () => {
		if (previous === undefined) delete process.env.AETHER_HOME;
		else process.env.AETHER_HOME = previous;
	};
}

test('a home under tools/ is accepted', () => {
	const restore = withHome(`${TOOLS}e2e\\home-replay-test`);
	try {
		assert.equal(requireTestHome(), `${TOOLS}e2e\\home-replay-test`);
	} finally {
		restore();
	}
});

test('the real profile of the player is refused', () => {
	const restore = withHome('C:\\Users\\ee');
	try {
		assert.throws(() => requireTestHome(), /outside tools\//);
	} finally {
		restore();
	}
});

test('an unset home is refused', () => {
	const restore = withHome(undefined);
	try {
		assert.throws(() => requireTestHome(), /AETHER_HOME is not set/);
	} finally {
		restore();
	}
});

test('a folder that only starts like tools/ is refused', () => {
	const sibling = TOOLS.replace(/[\\/]$/, '');
	const restore = withHome(`${sibling}-evil\\home`);
	try {
		assert.throws(() => requireTestHome(), /outside tools\//);
	} finally {
		restore();
	}
});

test('the home reported by the window is checked too', () => {
	const restore = withHome(`${TOOLS}e2e\\home-replay-test`);
	try {
		assert.equal(requireTestPaths({ paths: { home: `${TOOLS}e2e\\home-replay-test` } }), `${TOOLS}e2e\\home-replay-test`);
		assert.throws(() => requireTestPaths({ paths: { home: 'C:\\Users\\ee' } }), /real profile/);
		assert.throws(() => requireTestPaths({ paths: {} }), /did not report its home/);
	} finally {
		restore();
	}
});
