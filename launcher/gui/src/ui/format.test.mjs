// The pure helpers of the interface, checked without a browser.
//
// `format.js` is also loaded as a plain script by the shell, so these tests read the same file the
// interface runs instead of a copy.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { formatBytes, rangePercent } = require('./format.js');

test('a byte count keeps the unit the player reads', () => {
	assert.equal(formatBytes(0), '0 B');
	assert.equal(formatBytes(512), '512 B');
	assert.equal(formatBytes(1024), '1.0 KB');
	assert.equal(formatBytes(1536), '1.5 KB');
	assert.equal(formatBytes(10 * 1024), '10 KB');
	assert.equal(formatBytes(1024 * 1024), '1.0 MB');
	assert.equal(formatBytes(1024 * 1024 * 1024), '1.0 GB');
});

test('a byte count that is not a number is zero, not NaN', () => {
	assert.equal(formatBytes(undefined), '0 B');
	assert.equal(formatBytes(null), '0 B');
	assert.equal(formatBytes(NaN), '0 B');
});

test('the filled part of a slider follows its value', () => {
	assert.equal(rangePercent(0, 0, 100), 0);
	assert.equal(rangePercent(50, 0, 100), 50);
	assert.equal(rangePercent(100, 0, 100), 100);
	assert.ok(Math.abs(rangePercent('2048', '1024', '4096') - 100 / 3) < 1e-9);
});

test('a slider without a range is empty instead of dividing by zero', () => {
	assert.equal(rangePercent(5, 0, 0), 0);
	assert.equal(rangePercent(5, 10, 10), 0);
	assert.equal(rangePercent(5, 100, 0), 0);
});
