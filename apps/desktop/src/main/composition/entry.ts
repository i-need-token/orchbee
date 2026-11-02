/**
 * The Desktop application as a Server Client: one window whose document holds one Client connection
 * through an `orchbee server connect` relay.
 */
import { writeFileSync } from 'node:fs';
import path from 'node:path';

import {
	createOrchbeeIntl,
	desktopSecureKeyboardEntryMessages,
	desktopWindowMessages,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { app, net, protocol } from 'electron';

import { DesktopDevelopmentRestartRequestSchema } from '../../development';
import { resolveDesktopSessionName } from '../../distribution';
import { installDesktopApplicationMenu } from '../application-menu';
import {
	DESKTOP_RENDERER_URL,
	installDesktopRendererScheme,
	isPermittedDesktopRendererUrl,
	registerDesktopRendererScheme,
} from '../desktop-renderer-protocol';
import {
	DesktopSecureKeyboardEntry,
	DesktopSecureKeyboardEntryActions,
} from '../secure-keyboard-entry';
import { DesktopWindowActions } from '../window-actions';
import { DesktopWindowStateOwner, FileDesktopWindowStateStore } from '../window-state';
import { DesktopServerClientApplication } from './DesktopServerClientApplication';
import { type DesktopDataRoot, prepareDesktopDataRoot } from './prepareDesktopDataRoot';
import { resolveDesktopApplicationDistribution } from './resolveDesktopApplicationDistribution';

const CLIENT_EXECUTABLE_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_CLIENT_EXECUTABLE';
const RENDERER_ROOT_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_ROOT';
const RENDERER_URL_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_URL';
const SESSION_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_SESSION';
const WINDOW_HIDDEN_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_WINDOW_HIDDEN';
// Cross-runtime contract: search for `packaged-desktop-resources` before changing these values.
const PACKAGED_COHORT_DIRECTORY = 'cohort';
const PACKAGED_RENDERER_DIRECTORY = 'dist';

registerDesktopRendererScheme(protocol);
const distribution = resolveDesktopApplicationDistribution({
	applicationPath: app.getAppPath(),
	environment: process.env,
	packaged: app.isPackaged,
	platform: process.platform,
});
app.setName(distribution.applicationName);
const dataRoot = prepareDesktopDataRoot({
	distribution,
	homeDirectory: app.getPath('home'),
	platform: process.platform,
});
app.setPath('userData', dataRoot.path);

/**
 * What `orchbee` runs under. Outside a release, a Session it starts keeps this data directory's
 * preferences rather than the installed release's.
 */
function resolveClientEnvironment(root: DesktopDataRoot): NodeJS.ProcessEnv {
	if (process.env['ORCHBEE_CONFIG_FILE'] !== undefined || root.scope.kind === 'release') {
		return process.env;
	}
	const configurationFile = path.join(root.path, 'config.toml');
	// The runtime refuses an explicit configuration file that does not exist, so an empty one stands
	// for the defaults; one already there is the person's and stays as it is.
	try {
		writeFileSync(configurationFile, '', { encoding: 'utf8', flag: 'wx', mode: 0o600 });
	} catch (error) {
		if (!(error instanceof Error && 'code' in error && error.code === 'EEXIST')) throw error;
	}
	return { ...process.env, ORCHBEE_CONFIG_FILE: configurationFile };
}

function requiredEnvironment(key: string): string {
	const value = process.env[key];
	if (value === undefined || value.length === 0) throw new Error(`${key} is required.`);
	return value;
}

/** The bundled cohort's `orchbee`, or in a checkout the one its development runtime staged. */
function resolveClientExecutable(): string {
	if (!app.isPackaged) return requiredEnvironment(CLIENT_EXECUTABLE_ENVIRONMENT_KEY);
	return path.join(
		process.resourcesPath,
		PACKAGED_COHORT_DIRECTORY,
		process.platform === 'win32' ? 'orchbee.exe' : 'orchbee',
	);
}

async function resolveRendererUrl(): Promise<URL> {
	const developmentUrl = app.isPackaged ? undefined : process.env[RENDERER_URL_ENVIRONMENT_KEY];
	if (developmentUrl !== undefined) return new URL(developmentUrl);
	await installDesktopRendererScheme(
		protocol,
		app.isPackaged
			? path.join(process.resourcesPath, PACKAGED_RENDERER_DIRECTORY)
			: requiredEnvironment(RENDERER_ROOT_ENVIRONMENT_KEY),
		(fileUrl) => net.fetch(fileUrl),
	);
	return new URL(DESKTOP_RENDERER_URL);
}

/** The menu, its window Actions, and macOS Secure Keyboard Entry; returns their removal. */
function installPlatformActions(application: DesktopServerClientApplication): () => void {
	const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
	const windowActions = new DesktopWindowActions(application);
	const secureKeyboardEntry = new DesktopSecureKeyboardEntry(
		process.platform,
		{
			isEnabled: () => app.isSecureKeyboardEntryEnabled(),
			setEnabled: (enabled) => app.setSecureKeyboardEntryEnabled(enabled),
		},
		process.platform === 'darwin' && app.isActive(),
	);
	const secureKeyboardEntryActions = new DesktopSecureKeyboardEntryActions(secureKeyboardEntry);
	const activateSecureKeyboardEntry = (): void => secureKeyboardEntry.activate();
	const deactivateSecureKeyboardEntry = (): void => secureKeyboardEntry.deactivate();
	app.on('browser-window-focus', activateSecureKeyboardEntry);
	app.on('browser-window-blur', deactivateSecureKeyboardEntry);
	const menu = installDesktopApplicationMenu(
		{
			getSecureKeyboardEntryAvailability: () => secureKeyboardEntryActions.getToggleAvailability(),
			getToggleFullScreenAvailability: () => windowActions.getToggleFullScreenAvailability(),
			getToggleMaximizedAvailability: () => windowActions.getToggleMaximizedAvailability(),
			invokeToggleFullScreen: (source) => windowActions.invokeToggleFullScreen(source),
			invokeToggleMaximized: (source) => windowActions.invokeToggleMaximized(source),
			invokeToggleSecureKeyboardEntry: (source) => secureKeyboardEntryActions.invokeToggle(source),
		},
		{
			secureKeyboardEntry: intl.formatMessage(desktopSecureKeyboardEntryMessages.toggle),
			securityCategory: intl.formatMessage(desktopSecureKeyboardEntryMessages.category),
			toggleFullScreen: intl.formatMessage(desktopWindowMessages.toggleFullScreen),
			toggleMaximized: intl.formatMessage(desktopWindowMessages.toggleMaximized),
			windowCategory: intl.formatMessage(desktopWindowMessages.category),
		},
		process.platform,
	);
	return () => {
		app.off('browser-window-focus', activateSecureKeyboardEntry);
		app.off('browser-window-blur', deactivateSecureKeyboardEntry);
		menu.dispose();
		secureKeyboardEntryActions.dispose();
		windowActions.dispose();
		secureKeyboardEntry.dispose();
	};
}

let application: DesktopServerClientApplication | undefined;
let removePlatformActions: (() => void) | undefined;
let windowStateSettled = false;

async function start(): Promise<void> {
	const rendererUrl = await resolveRendererUrl();
	if (!isPermittedDesktopRendererUrl(rendererUrl)) {
		throw new Error('The Desktop renderer URL is not permitted.');
	}
	const session = process.env[SESSION_ENVIRONMENT_KEY] ?? resolveDesktopSessionName(distribution);
	const started = new DesktopServerClientApplication({
		clientEnvironment: resolveClientEnvironment(dataRoot),
		clientExecutable: resolveClientExecutable(),
		platform: process.platform,
		preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
		rendererUrl,
		sessionArguments: session === undefined ? [] : ['--session', session],
		windowHidden: process.env[WINDOW_HIDDEN_ENVIRONMENT_KEY] !== undefined,
		windowStateOwner: new DesktopWindowStateOwner(
			new FileDesktopWindowStateStore(path.join(dataRoot.path, 'window-state-v1.json')),
			process.platform,
		),
	});
	application = started;
	app.on('activate', () => void started.openWindow().catch(() => undefined));
	await started.openWindow();
	// The menu reads each Action's availability when it is installed and when it opens; a hidden
	// title bar never opens it, so it waits for the window its accelerators act on.
	removePlatformActions = installPlatformActions(started);
}

if (app.requestSingleInstanceLock()) {
	// Starting Orchbee again brings this window forward, or opens one where macOS kept none.
	app.on('second-instance', () => void application?.openWindow().catch(() => undefined));
	void app
		.whenReady()
		.then(start)
		.catch((error: unknown) => {
			console.error('Orchbee Desktop startup failed.', error);
			process.exitCode = 1;
			windowStateSettled = true;
			app.quit();
		});
} else {
	windowStateSettled = true;
	app.quit();
}

app.on('before-quit', (event) => {
	if (windowStateSettled || application === undefined) return;
	event.preventDefault();
	void application.settleWindowState().finally(() => {
		windowStateSettled = true;
		app.quit();
	});
});

app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') app.quit();
});

// The development runtime restarts Main after a rebuild through an ordinary quit, which records the
// window's place first; the Session keeps running, as it does after any quit.
if (!app.isPackaged && process.send !== undefined) {
	process.on('message', (message: unknown) => {
		if (DesktopDevelopmentRestartRequestSchema.safeParse(message).success) app.quit();
	});
}

app.once('will-quit', () => {
	removePlatformActions?.();
	removePlatformActions = undefined;
});
