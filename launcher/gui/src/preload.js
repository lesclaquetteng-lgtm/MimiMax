'use strict';

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('aether', {
	window: (action) => ipcRenderer.invoke('window:action', action),

	info: () => ipcRenderer.invoke('app:info'),
	copy: (text) => ipcRenderer.invoke('app:copy', text),

	settings: {
		get: () => ipcRenderer.invoke('settings:get'),
		update: (patch) => ipcRenderer.invoke('settings:update', patch)
	},

	accounts: {
		get: () => ipcRenderer.invoke('accounts:get'),
		addOffline: (username) => ipcRenderer.invoke('accounts:addOffline', username),
		remove: (id) => ipcRenderer.invoke('accounts:remove', id),
		select: (id) => ipcRenderer.invoke('accounts:select', id),
		microsoft: {
			signIn: () => ipcRenderer.invoke('accounts:microsoft:signIn')
		}
	},

	versions: {
		list: (limit) => ipcRenderer.invoke('versions:list', limit),
		loaders: (gameVersion) => ipcRenderer.invoke('versions:loaders', gameVersion)
	},

	profiles: {
		list: () => ipcRenderer.invoke('profiles:list'),
		assign: (accountId, name) => ipcRenderer.invoke('profiles:assign', accountId, name),
		add: (name, source) => ipcRenderer.invoke('profiles:add', name, source),
		update: (name, patch) => ipcRenderer.invoke('profiles:update', name, patch),
		remove: (name) => ipcRenderer.invoke('profiles:remove', name)
	},

	mods: {
		list: () => ipcRenderer.invoke('mods:list'),
		search: (query, gameVersion) => ipcRenderer.invoke('mods:search', query, gameVersion),
		install: (projectId, gameVersion) => ipcRenderer.invoke('mods:install', projectId, gameVersion),
		toggle: (name, enabled) => ipcRenderer.invoke('mods:toggle', name, enabled),
		openFolder: () => ipcRenderer.invoke('mods:openFolder')
	},

	servers: {
		list: () => ipcRenderer.invoke('servers:list'),
		add: (name, address) => ipcRenderer.invoke('servers:add', name, address),
		remove: (key) => ipcRenderer.invoke('servers:remove', key),
		ping: (address) => ipcRenderer.invoke('servers:ping', address),
		probe: (address) => ipcRenderer.invoke('servers:probe', address)
	},

	hosts: {
		list: () => ipcRenderer.invoke('hosts:list'),
		create: (options) => ipcRenderer.invoke('hosts:create', options),
		start: (name) => ipcRenderer.invoke('hosts:start', name),
		stop: (name) => ipcRenderer.invoke('hosts:stop', name)
	},

	performance: {
		list: () => ipcRenderer.invoke('performance:list'),
		install: (slugs) => ipcRenderer.invoke('performance:install', slugs),
		tune: (preset) => ipcRenderer.invoke('performance:tune', preset),
		jvm: (preset) => ipcRenderer.invoke('performance:jvm', preset)
	},

	logs: {
		tail: (lines) => ipcRenderer.invoke('logs:tail', lines),
		read: (name, lines) => ipcRenderer.invoke('logs:read', name, lines)
	},

	install: (profile) => ipcRenderer.invoke('install:start', profile),
	installStop: () => ipcRenderer.invoke('install:stop'),
	launch: (options) => ipcRenderer.invoke('game:launch', options),
	stop: () => ipcRenderer.invoke('game:stop'),

	onInstallProgress: (callback) => {
		ipcRenderer.on('install:progress', (_event, progress) => callback(progress));
	},
	onOptimisationProgress: (callback) => {
		ipcRenderer.on('optimisation:progress', (_event, progress) => callback(progress));
	},
	onModrinthProgress: (callback) => {
		ipcRenderer.on('mods:progress', (_event, progress) => callback(progress));
	},
	onGameOutput: (callback) => {
		ipcRenderer.on('game:output', (_event, line) => callback(line));
	},
	onMicrosoftDeviceCode: (callback) => {
		ipcRenderer.on('microsoft:deviceCode', (_event, code) => callback(code));
	}
});
