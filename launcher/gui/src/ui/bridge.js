/*
 * The bridge between the interface and the Rust shell.
 *
 * The interface calls `window.aether`, exactly like it did with the Electron preload script. This
 * file only exists in the Tauri build: when Electron already defined the object, or when the page
 * is opened in a plain browser, it does nothing at all.
 */
(() => {
	'use strict';

	const internals = window.__TAURI_INTERNALS__;
	if (!internals || window.aether) {
		return;
	}

	const invoke = (command, args) => internals.invoke(command, args || {});

	// Tauri hands the whole `{ event, id, payload }` envelope to the callback, the interface only
	// ever wanted the payload.
	const listen = (event, handler) => {
		const id = internals.transformCallback((message) => handler(message.payload));
		return invoke('plugin:event|listen', { event, target: { kind: 'Any' }, handler: id });
	};

	const copy = async (value) => {
		const text = String(value ?? '');

		try {
			await navigator.clipboard.writeText(text);
			return { ok: true };
		} catch {
			// WebView2 refuses the clipboard API when the document is not focused; the old
			// selection command still works and the report is worth copying.
			const area = document.createElement('textarea');
			area.value = text;
			area.setAttribute('readonly', '');
			area.style.position = 'fixed';
			area.style.top = '0';
			area.style.opacity = '0';
			document.body.appendChild(area);
			area.select();

			let copied = false;
			try {
				copied = document.execCommand('copy');
			} catch {
				copied = false;
			}
			area.remove();

			return copied
				? { ok: true }
				: { ok: false, error: 'The clipboard is unavailable.' };
		}
	};

	window.aether = {
		window: (action) => invoke('window_action', { action }),

		info: () => invoke('app_info'),
		copy,

		settings: {
			get: () => invoke('settings_get'),
			update: (patch) => invoke('settings_update', { patch })
		},

		accounts: {
			get: () => invoke('accounts_get'),
			addOffline: (username) => invoke('accounts_add_offline', { username }),
			checkName: (username) => invoke('accounts_check_name', { username }),
			remove: (id) => invoke('accounts_remove', { id }),
			select: (id) => invoke('accounts_select', { id }),
			microsoft: {
				signIn: () => invoke('accounts_microsoft_sign_in')
			}
		},

		versions: {
			list: (limit) => invoke('versions_list', { limit }),
			loaders: (gameVersion) => invoke('versions_loaders', { game_version: gameVersion })
		},

		profiles: {
			list: () => invoke('profiles_list'),
			assign: (accountId, name) => invoke('profiles_assign', { account_id: accountId, name }),
			add: (name, source) => invoke('profiles_add', { name, source }),
			update: (name, patch) => invoke('profiles_update', { name, patch }),
			remove: (name) => invoke('profiles_remove', { name })
		},

		mods: {
			list: () => invoke('mods_list'),
			search: (query, gameVersion) => invoke('mods_search', { query, game_version: gameVersion }),
			install: (projectId, gameVersion) => invoke('mods_install', { project_id: projectId, game_version: gameVersion }),
			toggle: (name, enabled) => invoke('mods_toggle', { name, enabled }),
			openFolder: () => invoke('mods_open_folder')
		},

		servers: {
			list: () => invoke('servers_list'),
			add: (name, address) => invoke('servers_add', { name, address }),
			remove: (key) => invoke('servers_remove', { key }),
			ping: (address) => invoke('servers_ping', { address }),
			probe: (address) => invoke('servers_probe', { address })
		},

		hosts: {
			list: () => invoke('hosts_list'),
			create: (options) => invoke('hosts_create', { options }),
			start: (name) => invoke('hosts_start', { name }),
			stop: (name) => invoke('hosts_stop', { name })
		},

		performance: {
			list: () => invoke('performance_list'),
			install: (slugs) => invoke('performance_install', { slugs }),
			tune: (preset) => invoke('performance_tune', { preset }),
			jvm: (preset) => invoke('performance_jvm', { preset })
		},

		logs: {
			tail: (lines) => invoke('logs_tail', { lines }),
			read: (name, lines) => invoke('logs_read', { name, lines })
		},

		install: (profile) => invoke('install_start', { profile }),
		installStop: () => invoke('install_stop'),
		launch: (options) => invoke('game_launch', { options }),
		stop: () => invoke('game_stop'),

		onInstallProgress: (callback) => listen('install:progress', callback),
		onOptimisationProgress: (callback) => listen('optimisation:progress', callback),
		onModrinthProgress: (callback) => listen('mods:progress', callback),
		onGameOutput: (callback) => listen('game:output', callback),
		onMicrosoftDeviceCode: (callback) => listen('microsoft:deviceCode', callback)
	};
})();
