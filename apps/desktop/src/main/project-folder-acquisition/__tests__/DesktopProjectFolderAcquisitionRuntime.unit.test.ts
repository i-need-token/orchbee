import type { BrowserWindow } from 'electron';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopProjectFolderAcquisitionRuntime,
	type DesktopProjectFolderAcquisitionRuntimeDependencies,
} from '../DesktopProjectFolderAcquisitionRuntime';
import {
	createProjectFolder,
	type NewProjectFolderCreationDependencies,
} from '../createProjectFolder';

describe('DesktopProjectFolderAcquisitionRuntime', () => {
	it('does not mkdir on invalid names or retirement before the syscall', async () => {
		const parent = resolve('/workspace');
		const mkdir = vi.fn<NewProjectFolderCreationDependencies['mkdir']>(async () => undefined);
		const resolveSelectedFolder = vi.fn<
			NewProjectFolderCreationDependencies['resolveSelectedFolder']
		>(async () => ({ kind: 'selected', path: parent }));
		await expect(
			createProjectFolder(parent, '../escape', () => true, { mkdir, resolveSelectedFolder }),
		).resolves.toMatchObject({ code: 'invalid-name', kind: 'failed' });
		expect(resolveSelectedFolder).not.toHaveBeenCalled();
		await expect(
			createProjectFolder(parent, 'new', () => false, { mkdir, resolveSelectedFolder }),
		).resolves.toMatchObject({ code: 'control-unavailable', kind: 'failed' });
		expect(mkdir).not.toHaveBeenCalled();
	});

	it('distinguishes an existing target from an unknown syscall result and cannot cancel committed creation', async () => {
		const parent = resolve('/workspace');
		const resolveSelectedFolder = vi.fn<
			NewProjectFolderCreationDependencies['resolveSelectedFolder']
		>(async (path) => ({ kind: 'selected', path }));
		const mkdir = vi
			.fn<NewProjectFolderCreationDependencies['mkdir']>()
			.mockRejectedValueOnce({ code: 'EEXIST' })
			.mockRejectedValueOnce({ code: 'EIO' });
		await expect(
			createProjectFolder(parent, 'new', () => true, { mkdir, resolveSelectedFolder }),
		).resolves.toEqual({ code: 'already-exists', kind: 'failed' });
		await expect(
			createProjectFolder(parent, 'new', () => true, { mkdir, resolveSelectedFolder }),
		).resolves.toEqual({ kind: 'creation-unknown', path: resolve(parent, 'new') });
		let live = true;
		mkdir.mockImplementation(async () => {
			live = false;
		});
		await expect(
			createProjectFolder(parent, 'new', () => live, { mkdir, resolveSelectedFolder }),
		).resolves.toEqual({ kind: 'created', path: resolve(parent, 'new') });
	});
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
