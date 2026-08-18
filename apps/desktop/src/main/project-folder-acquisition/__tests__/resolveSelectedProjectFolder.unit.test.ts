import { describe, expect, it, vi } from 'vitest';

import {
	resolveSelectedProjectFolder,
	type SelectedProjectFolderValidationDependencies,
} from '../resolveSelectedProjectFolder';

describe('resolveSelectedProjectFolder', () => {
	it('normalizes an absolute selection while preserving its symlink spelling', async () => {
		const dependencies = createDependencies();
		dependencies.resolve.mockReturnValue('/workspace/link');

		await expect(
			resolveSelectedProjectFolder('/workspace/parent/../link', dependencies),
		).resolves.toEqual({ kind: 'selected', path: '/workspace/link' });
		expect(dependencies.stat).toHaveBeenCalledWith('/workspace/link');
		expect(dependencies.access).toHaveBeenCalledWith('/workspace/link', expect.any(Number));
	});

	it('rejects relative paths, files, missing targets, and unreadable directories', async () => {
		const relative = createDependencies();
		relative.isAbsolute.mockReturnValue(false);
		await expect(resolveSelectedProjectFolder('relative', relative)).resolves.toEqual({
			code: 'validation-failed',
			kind: 'failed',
		});

		const file = createDependencies();
		file.stat.mockResolvedValue({ isDirectory: () => false });
		await expect(resolveSelectedProjectFolder('/workspace/file', file)).resolves.toEqual({
			code: 'not-directory',
			kind: 'failed',
		});

		const missing = createDependencies();
		missing.stat.mockRejectedValue(Object.assign(new Error('not found'), { code: 'ENOENT' }));
		await expect(resolveSelectedProjectFolder('/workspace/missing', missing)).resolves.toEqual({
			code: 'not-found',
			kind: 'failed',
		});

		const denied = createDependencies();
		denied.access.mockRejectedValue(Object.assign(new Error('denied'), { code: 'EACCES' }));
		await expect(resolveSelectedProjectFolder('/workspace/private', denied)).resolves.toEqual({
			code: 'permission-denied',
			kind: 'failed',
		});
	});

	it('treats disappearance during the access check and unexpected failures as bounded errors', async () => {
		const disappeared = createDependencies();
		disappeared.access.mockRejectedValue(Object.assign(new Error('gone'), { code: 'ENOENT' }));
		await expect(resolveSelectedProjectFolder('/workspace/gone', disappeared)).resolves.toEqual({
			code: 'not-found',
			kind: 'failed',
		});

		const unexpected = createDependencies();
		unexpected.stat.mockRejectedValue(new Error('raw filesystem detail'));
		await expect(resolveSelectedProjectFolder('/workspace/unknown', unexpected)).resolves.toEqual({
			code: 'validation-failed',
			kind: 'failed',
		});
	});
});

function createDependencies(): SelectedProjectFolderValidationDependencies & {
	readonly access: ReturnType<typeof vi.fn>;
	readonly isAbsolute: ReturnType<typeof vi.fn>;
	readonly resolve: ReturnType<typeof vi.fn>;
	readonly stat: ReturnType<typeof vi.fn>;
} {
	return {
		access: vi.fn<SelectedProjectFolderValidationDependencies['access']>(async () => undefined),
		isAbsolute: vi.fn<SelectedProjectFolderValidationDependencies['isAbsolute']>(() => true),
		resolve: vi.fn<SelectedProjectFolderValidationDependencies['resolve']>((path) => path),
		stat: vi.fn<SelectedProjectFolderValidationDependencies['stat']>(async () => ({
			isDirectory: () => true,
		})),
	};
}
