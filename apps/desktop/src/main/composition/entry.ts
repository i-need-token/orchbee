/**
 * The Desktop application as a Server Client: one window whose document holds one Client connection
 * through an `orchbee server connect` relay.
 */
import path from 'node:path';

import {
	createOrchbeeIntl,
	desktopDiagnosticsMessages,
	desktopSecureKeyboardEntryMessages,
	desktopWindowMessages,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { app, net, protocol } from 'electron';

import { DesktopDevelopmentRestartRequestSchema } from '../../development';
import { resolveDesktopCohortArtifactStem, resolveDesktopSessionName } from '../../distribution';
import { installDesktopApplicationMenu } from '../application-menu';
import {
	DesktopDiagnosticLog,
	DesktopDiagnosticsActions,
	DesktopDiagnosticsRuntime,
	type DesktopDiagnosticRecorder,
} from '../diagnostics';
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
/** What the Windows taskbar's Save Diagnostic Bundle task starts Orchbee with. */
const SAVE_DIAGNOSTIC_BUNDLE_SWITCH = '--save-diagnostic-bundle';
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
	applicationVersion: app.getVersion(),
	distribution,
	homeDirectory: app.getPath('home'),
	homeOverride: process.env['ORCHBEE_HOME'],
	platform: process.platform,
});
app.setPath('userData', dataRoot.electronDirectory);
// The natives write the same daily logs on Windows only until ORC-328, and so does Main.
const recordEvent: DesktopDiagnosticRecorder =
	process.platform === 'win32'
		? new DesktopDiagnosticLog(dataRoot.logDirectory).recorder()
		: () => undefined;

/**
 * What `orchbee` runs under: the same data root as Desktop, so a Session it starts keeps this
 * root's configuration and preferences rather than another environment's.
 */
function resolveClientEnvironment(root: DesktopDataRoot): NodeJS.ProcessEnv {
	// Windows natives are compiled with the profile's stem and derive the same root, and an inherited
	// ORCHBEE_HOME names it for both. Other platforms build release names until ORC-328, so a
	// development or test Desktop names its root for them.
	if (process.platform === 'win32' || root.scope.kind === 'release') return process.env;
	return { ...process.env, ORCHBEE_HOME: root.path };
}

function requiredEnvironment(key: string): string {
	const value = process.env[key];
	if (value === undefined || value.length === 0) throw new Error(`${key} is required.`);
	return value;
}

/** The bundled cohort's Client, or in a checkout the one its development runtime staged. */
function resolveClientExecutable(): string {
	if (!app.isPackaged) return requiredEnvironment(CLIENT_EXECUTABLE_ENVIRONMENT_KEY);
	const stem = resolveDesktopCohortArtifactStem(distribution, process.platform);
	return path.join(
		process.resourcesPath,
		PACKAGED_COHORT_DIRECTORY,
		process.platform === 'win32' ? `${stem}.exe` : stem,
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

/**
 * The menu, its window Actions, macOS Secure Keyboard Entry, and the Windows diagnostic bundle with
 * its taskbar task; returns their removal.
 */
function installPlatformActions(application: DesktopServerClientApplication): () => void {
	const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
	const windowActions = new DesktopWindowActions(application);
	const diagnosticsActions = new DesktopDiagnosticsActions(
		new DesktopDiagnosticsRuntime({
			clientEnvironment: resolveClientEnvironment(dataRoot),
			clientExecutable: resolveClientExecutable(),
			intl,
			recordEvent,
			window: () => application.window,
		}),
		process.platform,
	);
	saveDiagnosticBundle = () => diagnosticsActions.invokeSaveBundle('menu');
	// The title bar overlay hides the Windows menu bar, so the taskbar's Jump List holds the same
	// Action; a checkout's Electron has no identity of its own to hold one.
	if (process.platform === 'win32' && app.isPackaged) {
		app.setUserTasks([
			{
				arguments: SAVE_DIAGNOSTIC_BUNDLE_SWITCH,
				description: intl.formatMessage(desktopDiagnosticsMessages.taskDescription),
				iconIndex: 0,
				iconPath: process.execPath,
				program: process.execPath,
				title: intl.formatMessage(desktopDiagnosticsMessages.saveBundle),
			},
		]);
	}
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
			saveDiagnosticBundle: {
				categoryLabel: intl.formatMessage(desktopDiagnosticsMessages.category),
				getAvailability: () => diagnosticsActions.getSaveBundleAvailability(),
				invoke: (source) => diagnosticsActions.invokeSaveBundle(source),
				label: intl.formatMessage(desktopDiagnosticsMessages.saveBundle),
			},
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
		saveDiagnosticBundle = undefined;
		diagnosticsActions.dispose();
		secureKeyboardEntryActions.dispose();
		windowActions.dispose();
		secureKeyboardEntry.dispose();
	};
}

let application: DesktopServerClientApplication | undefined;
let removePlatformActions: (() => void) | undefined;
let saveDiagnosticBundle: (() => boolean) | undefined;
/** A Save Diagnostic Bundle task that started Orchbee waits for the Action to be installed. */
let diagnosticBundleRequested = process.argv.includes(SAVE_DIAGNOSTIC_BUNDLE_SWITCH);
let windowStateSettled = false;

/** Brings the window forward, so the save dialog opens over it, then starts the save. */
function requestDiagnosticBundle(): void {
	const save = saveDiagnosticBundle;
	if (save === undefined) {
		diagnosticBundleRequested = true;
		return;
	}
	void (application?.openWindow() ?? Promise.resolve()).then(save, save);
}

async function start(): Promise<void> {
	const rendererUrl = await resolveRendererUrl();
	if (!isPermittedDesktopRendererUrl(rendererUrl)) {
		throw new Error('The Desktop renderer URL is not permitted.');
	}
	const session = process.env[SESSION_ENVIRONMENT_KEY] ?? resolveDesktopSessionName(distribution);
	const started = new DesktopServerClientApplication({
		clientEnvironment: resolveClientEnvironment(dataRoot),
		clientExecutable: resolveClientExecutable(),
		distribution: distribution.dataScope.kind,
		platform: process.platform,
		preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
		recordEvent,
		rendererUrl,
		sessionArguments: session === undefined ? [] : ['--session', session],
		windowHidden: process.env[WINDOW_HIDDEN_ENVIRONMENT_KEY] !== undefined,
		windowStateOwner: new DesktopWindowStateOwner(
			new FileDesktopWindowStateStore(dataRoot.windowStateFile),
			process.platform,
		),
	});
	application = started;
	app.on('activate', () => void started.openWindow().catch(() => undefined));
	await started.openWindow();
	// The menu reads each Action's availability when it is installed and when it opens; a hidden
	// title bar never opens it, so it waits for the window its accelerators act on.
	removePlatformActions = installPlatformActions(started);
	if (diagnosticBundleRequested) {
		diagnosticBundleRequested = false;
		requestDiagnosticBundle();
	}
}

if (app.requestSingleInstanceLock()) {
	// Starting Orchbee again brings this window forward, or opens one where macOS kept none; the
	// taskbar's task also starts a diagnostic bundle save.
	app.on('second-instance', (_event, argv) => {
		if (argv.includes(SAVE_DIAGNOSTIC_BUNDLE_SWITCH)) requestDiagnosticBundle();
		else void application?.openWindow().catch(() => undefined);
	});
	recordEvent('info', 'started');
	void app
		.whenReady()
		.then(start)
		.catch((error: unknown) => {
			recordEvent('error', 'startup-failed');
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
	recordEvent('info', 'exited');
});
