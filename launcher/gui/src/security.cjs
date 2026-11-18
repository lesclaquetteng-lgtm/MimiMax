'use strict';

const MAX_EXTERNAL_URL_LENGTH = 2048;

/**
 * Only allow web links to leave the application. Reject credentials to avoid
 * passing crafted user-info URLs to the system browser.
 */
function normalizeExternalUrl(value) {
	if (typeof value !== 'string' || value.length === 0 || value.length > MAX_EXTERNAL_URL_LENGTH) {
		return null;
	}

	try {
		const url = new URL(value);
		if (url.protocol !== 'https:' || !url.hostname || url.username || url.password) {
			return null;
		}
		return url.toString();
	} catch {
		return null;
	}
}

/**
 * Keep the renderer inside its own local document. Hash/query changes are
 * allowed, but another file or any remote document is not.
 */
function isSameAppDocument(candidate, current) {
	try {
		const next = new URL(candidate);
		const active = new URL(current);
		return next.protocol === 'file:' &&
			active.protocol === 'file:' &&
			next.host === active.host &&
			next.pathname === active.pathname;
	} catch {
		return false;
	}
}

/**
 * Accept IPC only from the main window's top-level renderer frame.
 */
function isTrustedIpcEvent(event, window) {
	const contents = window?.webContents;
	const frame = event?.senderFrame;

	if (!contents || event.sender !== contents || !frame) return false;

	// WebFrameMain wrappers are not a reliable identity boundary across Electron
	// versions. Check the actual frame tree and its document instead: only the
	// top-level frame of this app's local index.html may invoke privileged IPC.
	if (frame.parent !== null) return false;

	return isSameAppDocument(frame.url, contents.getURL());
}

/**
 * Prevent two destructive long-running tasks of the same kind from racing
 * over the install directory or the active game process.
 */
function createOperationGate() {
	const active = new Set();

	return {
		isBusy(name) {
			return active.has(name);
		},
		async run(name, operation) {
			if (typeof name !== 'string' || !name || typeof operation !== 'function') {
				throw new TypeError('A gate name and operation function are required.');
			}
			if (active.has(name)) {
				return { ok: false, busy: true, error: `Another ${name} operation is already running.` };
			}

			active.add(name);
			try {
				return await operation();
			} finally {
				active.delete(name);
			}
		}
	};
}

module.exports = {
	createOperationGate,
	isSameAppDocument,
	isTrustedIpcEvent,
	normalizeExternalUrl
};
