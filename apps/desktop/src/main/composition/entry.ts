import path from 'node:path';

import { app } from 'electron';

import { DesktopApplication } from './DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

let desktopApplication: DesktopApplication | undefined;

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
			preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
			rendererFilePath: path.join(__dirname, '..', 'renderer', 'index.html'),
			...(rendererUrl === undefined ? {} : { rendererUrl }),
			runtime,
			traceCopyPath: process.env['ORCHBEE_DESKTOP_TRACE'] === '1',
		});
		await desktopApplication.start();
		app.on('activate', () => void desktopApplication?.openWindow());
	})
	.catch((error: unknown) => {
		console.error('Orchbee Desktop startup failed.', error);
		app.quit();
	});

app.on('before-quit', () => desktopApplication?.shutdown());
app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') app.quit();
});
