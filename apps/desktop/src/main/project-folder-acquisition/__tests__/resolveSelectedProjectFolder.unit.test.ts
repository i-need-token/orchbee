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
	});

	it('rejects relative paths', async () => {
		const relative = createDependencies();
		relative.isAbsolute.mockReturnValue(false);
		await expect(resolveSelectedProjectFolder('relative', relative)).resolves.toEqual({
			code: 'validation-failed',
			kind: 'failed',
		});
	});
});

function createDependencies(): SelectedProjectFolderValidationDependencies & {
	readonly isAbsolute: ReturnType<typeof vi.fn>;
	readonly resolve: ReturnType<typeof vi.fn>;
} {
	return {
		isAbsolute: vi.fn<SelectedProjectFolderValidationDependencies['isAbsolute']>(() => true),
		resolve: vi.fn<SelectedProjectFolderValidationDependencies['resolve']>((path) => path),
	};
}
