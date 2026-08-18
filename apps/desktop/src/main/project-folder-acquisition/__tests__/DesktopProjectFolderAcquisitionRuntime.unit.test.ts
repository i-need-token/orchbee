import type { BrowserWindow } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopProjectFolderAcquisitionRuntime,
	type DesktopProjectFolderAcquisitionRuntimeDependencies,
} from '../DesktopProjectFolderAcquisitionRuntime';

describe('DesktopProjectFolderAcquisitionRuntime', () => {
	it('opens one directory-only native dialog and validates its sole result', async () => {
		const dependencies = createDependencies();
		const runtime = new DesktopProjectFolderAcquisitionRuntime({} as BrowserWindow, dependencies);

		await expect(runtime.selectExistingFolder()).resolves.toEqual({
			kind: 'selected',
			path: '/workspace/project',
		});
		expect(dependencies.showOpenDialog).toHaveBeenCalledWith(expect.anything(), {
			properties: ['openDirectory'],
		});
		expect(dependencies.resolveSelectedFolder).toHaveBeenCalledWith('/workspace/project');
	});

	it('returns cancellation without validation and bounds dialog failures', async () => {
		const cancelled = createDependencies();
		cancelled.showOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
		const cancelledRuntime = new DesktopProjectFolderAcquisitionRuntime(
			{} as BrowserWindow,
			cancelled,
		);
		await expect(cancelledRuntime.selectExistingFolder()).resolves.toEqual({ kind: 'cancelled' });
		expect(cancelled.resolveSelectedFolder).not.toHaveBeenCalled();

		const failed = createDependencies();
		failed.showOpenDialog.mockRejectedValue(new Error('raw native detail'));
		const failedRuntime = new DesktopProjectFolderAcquisitionRuntime({} as BrowserWindow, failed);
		await expect(failedRuntime.selectExistingFolder()).resolves.toEqual({
			code: 'dialog-failed',
			kind: 'failed',
		});
	});

	it('rejects an ambiguous non-cancelled native result', async () => {
		const dependencies = createDependencies();
		dependencies.showOpenDialog.mockResolvedValue({
			canceled: false,
			filePaths: ['/workspace/one', '/workspace/two'],
		});
		const runtime = new DesktopProjectFolderAcquisitionRuntime({} as BrowserWindow, dependencies);

		await expect(runtime.selectExistingFolder()).resolves.toEqual({
			code: 'validation-failed',
			kind: 'failed',
		});
	});
});

function createDependencies(): DesktopProjectFolderAcquisitionRuntimeDependencies & {
	readonly resolveSelectedFolder: ReturnType<typeof vi.fn>;
	readonly showOpenDialog: ReturnType<typeof vi.fn>;
} {
	return {
		resolveSelectedFolder: vi.fn<
			DesktopProjectFolderAcquisitionRuntimeDependencies['resolveSelectedFolder']
		>(async (path) => ({ kind: 'selected', path })),
		showOpenDialog: vi.fn<DesktopProjectFolderAcquisitionRuntimeDependencies['showOpenDialog']>(
			async () => ({ canceled: false, filePaths: ['/workspace/project'] }),
		),
	};
}
