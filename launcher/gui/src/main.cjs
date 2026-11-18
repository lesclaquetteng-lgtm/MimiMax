const { app, BrowserWindow, clipboard, ipcMain, shell } = require('electron');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { existsSync, mkdirSync } = require('node:fs');
const { createOperationGate, isSameAppDocument, isTrustedIpcEvent, normalizeExternalUrl } = require('./security.cjs');

const here = __dirname;
const launcherRoot = path.resolve(here, '..', '..');

let mainWindow = null;
const operationGate = createOperationGate();

function handleIpc(channel, handler) {
	ipcMain.handle(channel, (event, ...args) => {
		if (!isTrustedIpcEvent(event, mainWindow)) {
			return { ok: false, error: 'Untrusted IPC sender.' };
		}
		return handler(event, ...args);
	});
}

function openExternalSafely(value) {
	const url = normalizeExternalUrl(value);
	if (url) void shell.openExternal(url).catch(() => {});
}

/**
 * The game that is running, so that the interface can stop it.
 */
let activeGame = null;

/**
 * The installation that is running, so that the interface can cancel it.
 */
let activeInstall = null;

/**
 * Kills a process and its children.
 *
 * The core starts Java as a child, stopping the core alone would leave the game running.
 */
function killProcessTree(pid) {
	if (!pid) return;

	if (process.platform === 'win32') {
		spawn('taskkill', ['/pid', String(pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
		return;
	}

	try {
		process.kill(-pid, 'SIGKILL');
	} catch {
		try {
			process.kill(pid, 'SIGKILL');
		} catch {
			// The process is already gone.
		}
	}
}

/**
 * The interface reads the store directly, the core answers with `{ ok, accounts }`.
 */
function accountStore(result) {
	const store =
		result && result.accounts && typeof result.accounts === 'object'
			? result.accounts
			: { accounts: [], selected: null };

	if (!result || !result.ok) {
		return {
			ok: false,
			error: result?.error || 'The accounts could not be read.',
			...store
		};
	}

	return { ok: true, ...store };
}

/**
 * Finds the Rust binary of the launcher.
 *
 * The binary is the core of the launcher, the interface only talks to it.
 */
function findCore() {
	const name = process.platform === 'win32' ? 'aether-launcher.exe' : 'aether-launcher';

	if (process.env.AETHER_LAUNCHER_BIN && existsSync(process.env.AETHER_LAUNCHER_BIN)) {
		return process.env.AETHER_LAUNCHER_BIN;
	}

	const candidates = [];

	// A packaged launcher ships the core next to the resources of the application.
	if (process.resourcesPath) {
		candidates.push(path.join(process.resourcesPath, name));
		candidates.push(path.join(process.resourcesPath, 'aether-launcher', name));
	}

	candidates.push(path.join(launcherRoot, 'target', 'release', name));
	candidates.push(path.join(launcherRoot, 'target', 'debug', name));

	return candidates.find((candidate) => existsSync(candidate)) || null;
}

/**
 * The time the core has to answer before it is considered stuck.
 *
 * The commands that read the network are given more room, the local ones only need a moment.
 */
function coreTimeout(args) {
	if (args[0] === 'versions' || args[0] === 'loaders') return 45000;
	if (args[0] === 'servers') return 30000;
	// The first start of a hosted server installs Fabric and the vanilla server.
	if (args[0] === 'hosts' && args[1] === 'start') return 900000;
	if (args[0] === 'performance' && args[1] === 'install') return 900000;
	return 20000;
}

/**
 * Runs the Rust core and answers with the parsed JSON document.
 *
 * A core that does not answer is killed: without it the promise never settles, and the interface
 * waits forever with an empty panel and no way to try again.
 */
function runCore(args) {
	return new Promise((resolve) => {
		const binary = findCore();
		if (!binary) {
			resolve({
				ok: false,
				error: 'The launcher core is missing. Build it with "cargo build --release".'
			});
			return;
		}

		let output = '';
		let settled = false;
		let timer = null;

		const answer = (result) => {
			if (settled) return;
			settled = true;
			if (timer) clearTimeout(timer);
			resolve(result);
		};

		const child = spawn(binary, args, { windowsHide: true });

		const limit = coreTimeout(args);
		timer = setTimeout(() => {
			killProcessTree(child.pid);
			answer({
				ok: false,
				error: `The launcher core did not answer within ${Math.round(limit / 1000)} s.`
			});
		}, limit);

		// The timer must not keep the application alive on its own.
		if (typeof timer.unref === 'function') timer.unref();

		child.stdout.on('data', (chunk) => {
			output += chunk.toString();
		});
		child.stderr.on('data', (chunk) => {
			output += chunk.toString();
		});
		child.on('error', (error) => {
			answer({ ok: false, error: error.message });
		});
		child.on('close', () => {
			const lines = output.split(/\r?\n/).filter((line) => line.trim().length > 0);
			let document = null;

			// The core answers with a single JSON document, the last one it printed wins.
			for (const line of lines) {
				try {
					const parsed = JSON.parse(line);
					if (parsed && typeof parsed === 'object') {
						document = parsed;
					}
				} catch {
					// not a JSON document, the raw output is reported below
				}
			}

			answer(
				document || {
					ok: false,
					error: output.trim() || 'The launcher core did not answer.'
				}
			);
		});
	});
}

/**
 * Runs the Rust core and forwards every line it prints, used for the installation and for the
 * output of the game.
 */
function runCoreStreaming(args, onLine, onSpawn) {
	return new Promise((resolve) => {
		const binary = findCore();
		if (!binary) {
			resolve({
				ok: false,
				error: 'The launcher core is missing. Build it with "cargo build --release".'
			});
			return;
		}

		const child = spawn(binary, args, {
			windowsHide: true,
			// On POSIX the whole group is killed, the game is a child of the core.
			detached: process.platform !== 'win32'
		});
		if (onSpawn) onSpawn(child);
		let buffer = '';
		let result = { ok: true };
		// The last document the core printed, the default answer is only a fallback.
		let document = null;

		const handle = (chunk) => {
			buffer += chunk.toString();
			const lines = buffer.split(/\r?\n/);
			buffer = lines.pop() || '';
			for (const line of lines) {
				if (line.trim().length === 0) continue;
														try {
						const parsed = JSON.parse(line);
						// A line with a type is an event, every other document is the answer.
						if (parsed.type) {
							onLine(parsed);
						} else {
							result = parsed;
							document = parsed;
						}
					} catch {
						onLine({ type: 'output', line });
					}
			}
		};

		child.stdout.on('data', handle);
		child.stderr.on('data', handle);
		child.on('error', (error) => resolve({ ok: false, error: error.message }));
		child.on('close', (code) => {
			if (buffer.trim().length > 0) {
				try {
					const parsed = JSON.parse(buffer.trim());
					result = parsed;
					document = parsed;
				} catch {
					onLine({ type: 'output', line: buffer.trim() });
				}
			}

			// A core that dies without a document must not be reported as a success: the default
			// answer would say "the game closed" for a crash that never ran anything. `crashed` lets
			// the interface tell this apart from a request the core refused.
			if (!document && typeof code === 'number' && code !== 0) {
				result = {
					ok: false,
					crashed: true,
					exitCode: code,
					error: `The launcher core stopped with the code ${code}.`
				};
			}

			resolve(result);
		});
	});
}

function createWindow() {
	mainWindow = new BrowserWindow({
		width: 1180,
		height: 760,
		minWidth: 940,
		minHeight: 620,
		backgroundColor: '#0e0f13',
		title: 'Aether Launcher',
		frame: false,
		show: false,
		webPreferences: {
			preload: path.join(here, 'preload.js'),
			contextIsolation: true,
			nodeIntegration: false,
			sandbox: false
		}
	});

	mainWindow.loadFile(path.join(here, 'ui', 'index.html'));
	mainWindow.once('ready-to-show', () => mainWindow.show());

	mainWindow.webContents.setWindowOpenHandler(({ url }) => {
		openExternalSafely(url);
		return { action: 'deny' };
	});

	mainWindow.webContents.on('will-navigate', (event, url) => {
		if (isSameAppDocument(url, mainWindow.webContents.getURL())) return;
		event.preventDefault();
		openExternalSafely(url);
	});
}

// Only one launcher may write the settings and the accounts of the player, two instances would
// overwrite each other.
const hasLock = app.requestSingleInstanceLock();

if (!hasLock) {
	app.quit();
} else {
	app.on('second-instance', () => {
		if (!mainWindow) return;
		if (mainWindow.isMinimized()) mainWindow.restore();
		mainWindow.focus();
	});

	app.whenReady().then(() => {
		registerHandlers();
		createWindow();

		app.on('activate', () => {
			if (BrowserWindow.getAllWindows().length === 0) {
				createWindow();
			}
		});
	});

	app.on('window-all-closed', () => {
		if (process.platform !== 'darwin') {
			app.quit();
		}
	});
}

function registerHandlers() {
	handleIpc('window:action', async (_event, action) => {
		if (!mainWindow) return;
		if (action === 'minimize') mainWindow.minimize();
		else if (action === 'maximize') mainWindow.isMaximized() ? mainWindow.unmaximize() : mainWindow.maximize();
		else if (action === 'close') mainWindow.close();
	});

	handleIpc('app:info', async () => runCore(['info']));

	// The report of a crash is copied from the main process, the clipboard of a `file://` page
	// is not reliable in every Electron build.
	handleIpc('app:copy', async (_event, text) => {
		clipboard.writeText(String(text ?? ''));
		return { ok: true };
	});

	handleIpc('settings:get', async () => runCore(['settings']));

	handleIpc('settings:update', async (_event, patch) => {
		const args = ['settings'];
		for (const [key, value] of Object.entries(patch || {})) {
			args.push('--key', key, '--value', String(value));
		}
		return runCore(args);
	});

	handleIpc('accounts:get', async () => accountStore(await runCore(['accounts'])));

	handleIpc('accounts:addOffline', async (_event, username) =>
		runCore(['account', 'add-offline', username]));

	handleIpc('accounts:remove', async (_event, id) =>
		accountStore(await runCore(['account', 'remove', id])));

	handleIpc('accounts:select', async (_event, id) =>
		accountStore(await runCore(['account', 'select', id])));

	handleIpc('accounts:microsoft:signIn', async () =>
		runCoreStreaming(['account', 'microsoft'], (line) => {
			if (line.type === 'deviceCode') {
				mainWindow?.webContents.send('microsoft:deviceCode', line);
			}
		}));

	handleIpc('versions:list', async (_event, limit) =>
		runCore(['versions', '--limit', String(limit || 40)]));

	handleIpc('versions:loaders', async (_event, gameVersion) =>
		runCore(['loaders', '--version', gameVersion]));

	handleIpc('profiles:list', async () => runCore(['profiles', 'list']));

	handleIpc('profiles:assign', async (_event, accountId, name) =>
		runCore(['profiles', 'assign', String(accountId), String(name)]));

	handleIpc('profiles:add', async (_event, name, source) =>
		runCore([
			'profiles', 'add', String(name),
			'--version', String(source.version),
			'--loader', String(source.loader),
			'--memory', String(source.memory),
			'--min-memory', String(source.minMemory),
			'--width', String(source.width),
			'--height', String(source.height)
		]));

	handleIpc('profiles:update', async (_event, name, patch) => {
		const flags = ['profiles', 'update', String(name)];

		for (const [key, flag] of [
			['version', '--version'],
			['loader', '--loader'],
			['memory', '--memory'],
			['minMemory', '--min-memory'],
			['width', '--width'],
			['height', '--height']
		]) {
			if (patch[key] !== undefined) flags.push(flag, String(patch[key]));
		}

		return runCore(flags);
	});

	handleIpc('profiles:remove', async (_event, name) =>
		runCore(['profiles', 'remove', String(name)]));

	handleIpc('mods:list', async () => runCore(['mods', 'list']));

	handleIpc('mods:toggle', async (_event, name, enabled) =>
		runCore(['mods', 'toggle', String(name), '--enabled', String(Boolean(enabled))]));

	// Opening a folder is a shell action, the core only says where the folder is.
	handleIpc('mods:openFolder', async () => {
		const info = await runCore(['info']);
		const folder = info?.paths?.mods;
		if (!folder) return { ok: false, error: 'The mods folder is unknown.' };

		try {
			mkdirSync(folder, { recursive: true });
		} catch (error) {
			return { ok: false, error: error.message };
		}

		const failure = await shell.openPath(folder);
		return failure ? { ok: false, error: failure, folder } : { ok: true, folder };
	});

	handleIpc('logs:tail', async (_event, lines) =>
		runCore(['logs', '--lines', String(lines || 200)]));	handleIpc('logs:read', async (_event, name, lines) =>
		runCore(['logs', 'read', String(name), '--lines', String(lines || 400)]));

	/* ------------------------------------------------------------- servers */

	handleIpc('servers:list', async () => runCore(['servers', 'list']));

	handleIpc('servers:add', async (_event, name, address) =>
		runCore(['servers', 'add', String(address), '--name', String(name || address)]));

	handleIpc('servers:remove', async (_event, key) =>
		runCore(['servers', 'remove', String(key)]));

	handleIpc('servers:ping', async (_event, address) =>
		runCore(['servers', 'ping', String(address)]));

	handleIpc('servers:probe', async (_event, address) =>
		runCore(['servers', 'probe', String(address)]));

	handleIpc('hosts:list', async () => runCore(['hosts', 'list']));

	handleIpc('hosts:create', async (_event, options) => {
		const args = [
			'hosts', 'create', String(options?.name || ''),
			'--port', String(options?.port || 25565),
			'--max-players', String(options?.maxPlayers || 20)
		];
		if (options?.motd) args.push('--motd', String(options.motd));
		if (options?.onlineMode) args.push('--online-mode', 'true');
		return runCore(args);
	});

	handleIpc('hosts:start', async (_event, name) =>
		runCore(['hosts', 'start', String(name)]));

	handleIpc('hosts:stop', async (_event, name) =>
		runCore(['hosts', 'stop', String(name)]));

	/* --------------------------------------------------------- performance */

	handleIpc('performance:list', async () => runCore(['performance', 'list']));

	handleIpc('performance:tune', async (_event, preset) =>
		runCore(['performance', 'tune', '--preset', String(preset || 'max')]));

	handleIpc('performance:jvm', async (_event, preset) =>
		runCore(['performance', 'jvm', String(preset || 'balanced')]));

	handleIpc('performance:install', async (_event, slugs) =>
		operationGate.run('install', async () => {
			const args = ['performance', 'install'];
			if (Array.isArray(slugs) && slugs.length > 0) args.push('--slugs', slugs.join(','));

			return runCoreStreaming(args, (line) => {
				if (line.type === 'optimisation') {
					mainWindow?.webContents.send('optimisation:progress', line);
				}
			});
		})
	);

	handleIpc('install:start', async (_event, profile) =>
		operationGate.run('install', async () => {
			const args = ['install'];
			if (profile?.gameVersion) args.push('--version', profile.gameVersion);
			if (profile?.loaderVersion) args.push('--loader', profile.loaderVersion);

			// The core reads `cancel` on its standard input while the installation runs.
			const installation = { child: null, cancelled: false };
			activeInstall = installation;

			try {
				const result = await runCoreStreaming(
					args,
					(line) => {
						if (line.type === 'progress') {
							mainWindow?.webContents.send('install:progress', line);
						}
					},
					(child) => {
						installation.child = child;
					}
				);

				return installation.cancelled
					? { ok: false, cancelled: true, error: 'The installation was cancelled.' }
					: result;
			} finally {
				if (activeInstall === installation) activeInstall = null;
			}
		})
	);

	handleIpc('install:stop', async () => {
		const installation = activeInstall;
		if (!installation || !installation.child) {
			return { ok: false, error: 'No installation is running.' };
		}

		installation.cancelled = true;

		try {
			installation.child.stdin.write('cancel\n');
		} catch {
			// The core already stopped, the answer of the installation says why.
		}

		// A core that ignores the request is stopped the hard way; the files it downloaded stay,
		// the next installation resumes them.
		setTimeout(() => {
			const child = installation.child;
			if (child && child.exitCode === null) killProcessTree(child.pid);
		}, 10000).unref?.();

		return { ok: true };
	});

	handleIpc('game:launch', async (_event, options) =>
		operationGate.run('game', async () => {
			const args = ['launch'];
			if (options?.accountId) args.push('--account', options.accountId);
			if (options?.gameVersion) args.push('--version', options.gameVersion);
			if (options?.server) args.push('--server', String(options.server));

			const game = { child: null, stopped: false };
			activeGame = game;

			try {
				const result = await runCoreStreaming(
					args,
					(line) => {
						if (line.type === 'output') {
							mainWindow?.webContents.send('game:output', line.line);
						}
					},
					(child) => {
						game.child = child;
					}
				);

				return game.stopped ? { ok: true, stopped: true } : result;
			} finally {
				if (activeGame === game) activeGame = null;
			}
		})
	);

	handleIpc('game:stop', async () => {
		if (!activeGame || !activeGame.child || !activeGame.child.pid) {
			return { ok: false, error: 'No game is running.' };
		}

		activeGame.stopped = true;
		killProcessTree(activeGame.child.pid);
		return { ok: true };
	});
}
