import path from 'node:path';
import { hostname } from 'node:os';

import {
	createOrchbeeIntl,
	desktopSecureKeyboardEntryMessages,
	desktopWindowMessages,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { app, net, protocol } from 'electron';

import {
	DESKTOP_DEVELOPMENT_RESTART_PLAN_ENVIRONMENT_KEY,
	DesktopDevelopmentRestartPlanStore,
	DesktopDevelopmentRestartRequestSchema,
} from '../../development';
import {
	DESKTOP_RENDERER_URL,
	installDesktopRendererScheme,
	registerDesktopRendererScheme,
} from '../desktop-renderer-protocol';
import { installDesktopApplicationMenu } from '../application-menu';
import {
	DesktopSecureKeyboardEntry,
	DesktopSecureKeyboardEntryActions,
} from '../secure-keyboard-entry';
import { DesktopWindowActions } from '../window-actions';
import { DesktopApplication } from './DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

let desktopApplication: DesktopApplication | undefined;
let desktopApplicationMenu: ReturnType<typeof installDesktopApplicationMenu> | undefined;
let desktopSecureKeyboardEntry: DesktopSecureKeyboardEntry | undefined;
let desktopSecureKeyboardEntryActions: DesktopSecureKeyboardEntryActions | undefined;
let desktopSecureKeyboardEntryBlurListener: (() => void) | undefined;
let desktopSecureKeyboardEntryFocusListener: (() => void) | undefined;
let desktopWindowActions: DesktopWindowActions | undefined;
let developmentRestartPlanStore: DesktopDevelopmentRestartPlanStore | undefined;
let bypassQuitGate = false;
registerDesktopRendererScheme(protocol);
const singleInstanceLockOwned = app.requestSingleInstanceLock();

if (!singleInstanceLockOwned) {
	bypassQuitGate = true;
	app.quit();
} else {
	app.on('second-instance', () => {
		if (desktopApplication?.hasWindow) {
			desktopApplication.focusWindow();
		} else {
			void desktopApplication?.openWindow().catch(() => undefined);
		}
	});

	void app
		.whenReady()
		.then(async () => {
			let rendererUrl: string;
			if (app.isPackaged) {
				await installDesktopRendererScheme(
					protocol,
					path.join(process.resourcesPath, 'dist'),
					(fileUrl) => net.fetch(fileUrl),
				);
				rendererUrl = DESKTOP_RENDERER_URL;
			} else {
				const developmentRendererUrl = process.env['ORCHBEE_DESKTOP_RENDERER_URL'];
				if (developmentRendererUrl === undefined) {
					throw new Error('ORCHBEE_DESKTOP_RENDERER_URL is required in development.');
				}
				rendererUrl = developmentRendererUrl;
			}
			let posixRuntimeBaseDirectory: string | undefined;
			if (process.platform === 'linux') {
				const getEffectiveUserId = process.geteuid;
				if (getEffectiveUserId === undefined) {
					throw new Error('Linux effective-user identity is unavailable.');
				}
				posixRuntimeBaseDirectory = `/run/user/${getEffectiveUserId()}`;
			} else if (process.platform !== 'win32') {
				posixRuntimeBaseDirectory = app.getPath('temp');
			}
			const runtime = resolveDesktopRuntimeConfiguration({
				applicationPath: app.getAppPath(),
				...(app.isPackaged
					? {}
					: {
							developmentShellIntegrationDirectory: path.resolve(
								__dirname,
								'..',
								'..',
								'resources',
								'shell-integration',
							),
						}),
				environment: process.env,
				homeDirectory: app.getPath('home'),
				...(app.isPackaged ? { packagedResourcesPath: process.resourcesPath } : {}),
				platform: process.platform,
				posixRuntimeBaseDirectory,
				terminalHostName: hostname(),
				userDataPath: app.getPath('userData'),
			});
			if (!app.isPackaged) {
				const restartPlanPath = process.env[DESKTOP_DEVELOPMENT_RESTART_PLAN_ENVIRONMENT_KEY];
				if (restartPlanPath !== undefined) {
					developmentRestartPlanStore = new DesktopDevelopmentRestartPlanStore(restartPlanPath);
				}
			}
			desktopApplication = new DesktopApplication({
				...(developmentRestartPlanStore === undefined
					? {}
					: {
							prepareDevelopmentHostRestart: (documentRevision) =>
								developmentRestartPlanStore?.markReady(documentRevision) ?? Promise.resolve(false),
						}),
				platform: process.platform,
				preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
				rendererUrl,
				runtime,
				singleInstanceLockOwned,
				traceCopyPath: process.env['ORCHBEE_DESKTOP_TRACE'] === '1',
			});
			await desktopApplication.start();
			const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
			desktopWindowActions = new DesktopWindowActions(desktopApplication);
			desktopSecureKeyboardEntry = new DesktopSecureKeyboardEntry(
				process.platform,
				{
					isEnabled: () => app.isSecureKeyboardEntryEnabled(),
					setEnabled: (enabled) => app.setSecureKeyboardEntryEnabled(enabled),
				},
				process.platform === 'darwin' && app.isActive(),
			);
			desktopSecureKeyboardEntryActions = new DesktopSecureKeyboardEntryActions(
				desktopSecureKeyboardEntry,
			);
			desktopSecureKeyboardEntryFocusListener = () => desktopSecureKeyboardEntry?.activate();
			desktopSecureKeyboardEntryBlurListener = () => desktopSecureKeyboardEntry?.deactivate();
			app.on('browser-window-focus', desktopSecureKeyboardEntryFocusListener);
			app.on('browser-window-blur', desktopSecureKeyboardEntryBlurListener);
			const windowActions = desktopWindowActions;
			const secureKeyboardEntryActions = desktopSecureKeyboardEntryActions;
			desktopApplicationMenu = installDesktopApplicationMenu(
				{
					getSecureKeyboardEntryAvailability: () =>
						secureKeyboardEntryActions.getToggleAvailability(),
					getToggleFullScreenAvailability: () => windowActions.getToggleFullScreenAvailability(),
					getToggleMaximizedAvailability: () => windowActions.getToggleMaximizedAvailability(),
					invokeToggleFullScreen: (source) => windowActions.invokeToggleFullScreen(source),
					invokeToggleMaximized: (source) => windowActions.invokeToggleMaximized(source),
					invokeToggleSecureKeyboardEntry: (source) =>
						secureKeyboardEntryActions.invokeToggle(source),
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
			app.on('activate', () => void desktopApplication?.openWindow());
		})
		.catch((error: unknown) => {
			console.error('Orchbee Desktop startup failed.', error);
			bypassQuitGate = true;
			app.quit();
		});
}

if (!app.isPackaged) {
	process.on('message', (message: unknown) => {
		const parsed = DesktopDevelopmentRestartRequestSchema.safeParse(message);
		if (!parsed.success || desktopApplication === undefined) return;
		if (parsed.data.reason === 'terminal-host' && developmentRestartPlanStore === undefined) {
			return;
		}
		console.info(
			`Orchbee Desktop received development restart request ${parsed.data.generation} (${parsed.data.reason}).`,
		);
		void desktopApplication
			.requestExitAndQuit(
				parsed.data.reason === 'terminal-host' ? 'development-host-restart' : 'update-restart',
			)
			.then(() => {
				if (desktopApplication?.quitPrepared !== true) {
					console.warn('Orchbee Desktop development restart did not reach a ready exit.');
				}
			})
			.catch((error: unknown) => {
				console.error('Orchbee Desktop development restart failed.', error);
			});
	});
}

app.on('before-quit', (event) => {
	if (bypassQuitGate || desktopApplication === undefined || desktopApplication.quitPrepared) {
		return;
	}
	event.preventDefault();
	void desktopApplication?.requestExitAndQuit('user-quit');
});

app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') app.quit();
});

app.once('will-quit', () => {
	if (desktopSecureKeyboardEntryFocusListener !== undefined) {
		app.off('browser-window-focus', desktopSecureKeyboardEntryFocusListener);
		desktopSecureKeyboardEntryFocusListener = undefined;
	}
	if (desktopSecureKeyboardEntryBlurListener !== undefined) {
		app.off('browser-window-blur', desktopSecureKeyboardEntryBlurListener);
		desktopSecureKeyboardEntryBlurListener = undefined;
	}
	desktopApplicationMenu?.dispose();
	desktopApplicationMenu = undefined;
	desktopSecureKeyboardEntryActions?.dispose();
	desktopSecureKeyboardEntryActions = undefined;
	desktopWindowActions?.dispose();
	desktopWindowActions = undefined;
	desktopSecureKeyboardEntry?.dispose();
	desktopSecureKeyboardEntry = undefined;
});
