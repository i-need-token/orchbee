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
	DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL,
	DesktopServerConnectionDocumentSchema,
	type DesktopServerConnectionReply,
} from '@orchbee/desktop-client-protocol';
import { BrowserWindow, MessageChannelMain, app, net, protocol, shell } from 'electron';

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
/** What a terminal link may open, as the page's own link policy permits. */
const EXTERNAL_LINK_PROTOCOLS = new Set(['http:', 'https:', 'mailto:']);

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

/** A link the page opens asks for a window; the person's browser takes it instead. */
function openExternalLink(address: string): void {
	let url: URL;
	try {
		url = new URL(address);
	} catch {
		return;
	}
	if (!EXTERNAL_LINK_PROTOCOLS.has(url.protocol)) return;
	// The page has already counted the link as opened, as the TUI counts an accepted request.
	shell.openExternal(url.href).catch(() => undefined);
}

function sessionArguments(): readonly string[] {
	const session = process.env[SESSION_ENVIRONMENT_KEY];
	return session === undefined ? [] : ['--session', session];
}

function connectDocument(
	window: BrowserWindow,
	reply: DesktopServerConnectionReply,
): ServerConnectionRelay {
	const relayProcess = spawn(
		requiredEnvironment(CLIENT_EXECUTABLE_ENVIRONMENT_KEY),
		[...sessionArguments(), 'server', 'connect'],
		{ stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true },
	);
	const { port1, port2 } = new MessageChannelMain();
	const relay = new ServerConnectionRelay(relayProcess, port1);
	window.webContents.postMessage(DESKTOP_SERVER_CONNECTION_IPC_CHANNEL, reply, [port2]);
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
	window.webContents.setWindowOpenHandler(({ url }) => {
		openExternalLink(url);
		return { action: 'deny' };
	});
	window.webContents.on('will-navigate', (event, navigationUrl) => {
		if (!isSameDesktopRendererOrigin(navigationUrl, rendererUrl)) event.preventDefault();
	});
	// A document asks when it is listening, and its asking ends the connection of the one it replaced.
	let relay: ServerConnectionRelay | undefined;
	let requests = 0;
	let everConnected = false;
	window.webContents.ipc.on(DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL, (_event, value) => {
		const document = DesktopServerConnectionDocumentSchema.safeParse(value);
		if (!document.success) return;
		relay?.close();
		relay = undefined;
		requests += 1;
		const request = requests;
		void readServerRestoreState(
			requiredEnvironment(CLIENT_EXECUTABLE_ENVIRONMENT_KEY),
			sessionArguments(),
		).then((restore) => {
			if (request !== requests || window.isDestroyed()) return;
			relay = connectDocument(window, {
				bootstrap: {
					// A later document follows a person's own actions; an empty Session may be what they chose.
					automaticWorkspaceAllowed: !everConnected && restore === 'intact',
					defaultDirectory: app.getPath('home'),
					terminalHostName: hostname(),
				},
				document: document.data,
			});
			everConnected = true;
		});
	});
	window.on('closed', () => {
		requests += 1;
		relay?.close();
	});
	await window.loadURL(rendererUrl.href);
});

app.on('window-all-closed', () => app.quit());
