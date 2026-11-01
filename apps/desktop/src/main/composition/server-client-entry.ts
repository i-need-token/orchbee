/**
 * The Desktop application as a Server Client: one window whose document holds one Client connection
 * through an `orchbee server connect` relay. It runs beside the frozen Main entry until the old entry
 * is removed.
 */
import { spawn } from 'node:child_process';
import { hostname } from 'node:os';
import path from 'node:path';

import {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	type DesktopServerConnectionBootstrap,
} from '@orchbee/desktop-client-protocol';
import { BrowserWindow, MessageChannelMain, app, net, protocol } from 'electron';

import {
	DESKTOP_RENDERER_URL,
	installDesktopRendererScheme,
	isPermittedDesktopRendererUrl,
	isSameDesktopRendererOrigin,
	registerDesktopRendererScheme,
} from '../desktop-renderer-protocol';
import { ServerConnectionRelay, readServerRestoreState } from '../server-connection';
import { prepareDesktopDataRoot } from './prepareDesktopDataRoot';
import { resolveDesktopApplicationDistribution } from './resolveDesktopApplicationDistribution';

const CLIENT_EXECUTABLE_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_CLIENT_EXECUTABLE';
const RENDERER_ROOT_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_ROOT';
const RENDERER_URL_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_URL';
const SESSION_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_SESSION';
const WINDOW_HIDDEN_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_WINDOW_HIDDEN';
/** Tells the shared page that it is a Server Client rather than the frozen Main entry's renderer. */
const DESKTOP_SERVER_CLIENT_PARAMETER = 'orchbee-server-client';

registerDesktopRendererScheme(protocol);
const distribution = resolveDesktopApplicationDistribution({
	applicationPath: app.getAppPath(),
	environment: process.env,
	packaged: app.isPackaged,
	platform: process.platform,
});
app.setName(distribution.applicationName);
app.setPath(
	'userData',
	prepareDesktopDataRoot({
		distribution,
		homeDirectory: app.getPath('home'),
		platform: process.platform,
	}).path,
);

function requiredEnvironment(key: string): string {
	const value = process.env[key];
	if (value === undefined || value.length === 0) throw new Error(`${key} is required.`);
	return value;
}

async function resolveRendererUrl(): Promise<URL> {
	const developmentUrl = process.env[RENDERER_URL_ENVIRONMENT_KEY];
	if (developmentUrl !== undefined) return new URL(developmentUrl);
	await installDesktopRendererScheme(
		protocol,
		requiredEnvironment(RENDERER_ROOT_ENVIRONMENT_KEY),
		(fileUrl) => net.fetch(fileUrl),
	);
	return new URL(DESKTOP_RENDERER_URL);
}

function sessionArguments(): readonly string[] {
	const session = process.env[SESSION_ENVIRONMENT_KEY];
	return session === undefined ? [] : ['--session', session];
}

function connectDocument(
	window: BrowserWindow,
	bootstrap: DesktopServerConnectionBootstrap,
): ServerConnectionRelay {
	const relayProcess = spawn(
		requiredEnvironment(CLIENT_EXECUTABLE_ENVIRONMENT_KEY),
		[...sessionArguments(), 'server', 'connect'],
		{ stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true },
	);
	const { port1, port2 } = new MessageChannelMain();
	const relay = new ServerConnectionRelay(relayProcess, port1);
	window.webContents.postMessage(DESKTOP_SERVER_CONNECTION_IPC_CHANNEL, bootstrap, [port2]);
	return relay;
}

void app.whenReady().then(async () => {
	const rendererUrl = await resolveRendererUrl();
	if (!isPermittedDesktopRendererUrl(rendererUrl)) {
		throw new Error('The Desktop renderer URL is not permitted.');
	}
	rendererUrl.searchParams.set(DESKTOP_SERVER_CLIENT_PARAMETER, '1');
	const window = new BrowserWindow({
		height: 720,
		show: process.env[WINDOW_HIDDEN_ENVIRONMENT_KEY] === undefined,
		webPreferences: {
			allowRunningInsecureContent: false,
			// A document behind other windows still has to keep its terminal current.
			backgroundThrottling: false,
			contextIsolation: true,
			nodeIntegration: false,
			preload: path.join(__dirname, '..', 'preload', 'preload.cjs'),
			sandbox: true,
			spellcheck: false,
			webSecurity: true,
		},
		width: 1100,
	});
	window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
	window.webContents.on('will-navigate', (event, navigationUrl) => {
		if (!isSameDesktopRendererOrigin(navigationUrl, rendererUrl)) event.preventDefault();
	});
	// A reload's document gets its own connection; the old one ends with the document that held it.
	let relay: ServerConnectionRelay | undefined;
	let documents = 0;
	window.webContents.on('did-finish-load', () => {
		relay?.close();
		relay = undefined;
		documents += 1;
		const document = documents;
		void readServerRestoreState(
			requiredEnvironment(CLIENT_EXECUTABLE_ENVIRONMENT_KEY),
			sessionArguments(),
		).then((restore) => {
			if (document !== documents || window.isDestroyed()) return;
			relay = connectDocument(window, {
				// A later document follows a person's own actions; an empty Session may be what they chose.
				automaticWorkspaceAllowed: document === 1 && restore === 'intact',
				defaultDirectory: app.getPath('home'),
				terminalHostName: hostname(),
			});
		});
	});
	window.on('closed', () => {
		documents += 1;
		relay?.close();
	});
	await window.loadURL(rendererUrl.href);
});

app.on('window-all-closed', () => app.quit());
