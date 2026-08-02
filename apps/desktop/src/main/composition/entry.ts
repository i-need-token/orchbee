import path from 'node:path';

import { app } from 'electron';

import { DesktopApplication } from './DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

let desktopApplication: DesktopApplication | undefined;
let bypassQuitGate = false;
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
			const rendererUrl = process.env['ORCHBEE_DESKTOP_RENDERER_URL'];
			const runtime = resolveDesktopRuntimeConfiguration({
				applicationPath: app.getAppPath(),
				environment: process.env,
				homeDirectory: app.getPath('home'),
				platform: process.platform,
				userDataPath: app.getPath('userData'),
			});
			desktopApplication = new DesktopApplication({
				platform: process.platform,
				preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
				rendererFilePath: path.join(__dirname, '..', 'renderer', 'index.html'),
				...(rendererUrl === undefined ? {} : { rendererUrl }),
				runtime,
				singleInstanceLockOwned,
				traceCopyPath: process.env['ORCHBEE_DESKTOP_TRACE'] === '1',
			});
			await desktopApplication.start();
			app.on('activate', () => void desktopApplication?.openWindow());
		})
		.catch((error: unknown) => {
			console.error('Orchbee Desktop startup failed.', error);
			bypassQuitGate = true;
			app.quit();
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
