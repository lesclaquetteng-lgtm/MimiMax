/**
 * The pure formatting helpers of the launcher interface.
 *
 * They live in their own file so that `node --test` can check them without a window, WebView2 or an
 * end-to-end run: a byte count and a slider percentage are arithmetic, and the end-to-end scripts
 * were the only thing proving them before. The file is a plain script for the shell and a CommonJS
 * module for the tests.
 */
(function (root, factory) {
	const api = factory();
	if (typeof module === 'object' && module.exports) {
		module.exports = api;
	} else {
		root.AetherFormat = api;
	}
})(typeof globalThis === 'object' ? globalThis : this, function () {
	/** A byte count as the interface shows it: 0 B, 512 B, 1.5 KB, 10 MB. */
	function formatBytes(bytes) {
		if (!bytes) return '0 B';
		const units = ['B', 'KB', 'MB', 'GB'];
		let index = 0;
		let value = bytes;
		while (value >= 1024 && index < units.length - 1) {
			value /= 1024;
			index++;
		}
		return `${value.toFixed(value >= 10 || index === 0 ? 0 : 1)} ${units[index]}`;
	}

	/** A number, or the fallback when the attribute is absent or not a number. */
	function number(value, fallback) {
		if (value === '' || value === null || value === undefined) return fallback;
		const parsed = Number(value);
		return Number.isFinite(parsed) ? parsed : fallback;
	}

	/** The filled part of a slider, in percent of its range. */
	function rangePercent(value, min, max) {
		const low = number(min, 0);
		const high = number(max, 100);
		const span = high - low;
		return span > 0 ? ((number(value, 0) - low) / span) * 100 : 0;
	}

	return { formatBytes, rangePercent };
});
