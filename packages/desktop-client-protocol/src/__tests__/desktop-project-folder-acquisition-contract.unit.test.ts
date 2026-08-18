import { describe, expect, it } from 'vitest';

import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
} from '../desktop-project-folder-acquisition-contract';

describe('Desktop Project Folder-acquisition contract', () => {
	it('correlates one bounded selection request with a selected path', () => {
		const command = DesktopProjectFolderAcquisitionRendererCommandSchema.parse({
			requestId: 'project-folder:request:1',
			type: 'desktop-project-folder-selection-requested',
		});

		expect(
			DesktopProjectFolderAcquisitionMainEventSchema.parse({
				requestId: command.requestId,
				result: { kind: 'selected', path: '/workspace/orchbee' },
				type: 'desktop-project-folder-acquisition-completed',
			}),
		).toMatchObject({ result: { kind: 'selected', path: '/workspace/orchbee' } });
	});

	it('represents cancellation without a path and rejects unbounded details', () => {
		expect(
			DesktopProjectFolderAcquisitionMainEventSchema.parse({
				requestId: 'project-folder:request:2',
				result: { kind: 'cancelled' },
				type: 'desktop-project-folder-acquisition-completed',
			}),
		).toMatchObject({ result: { kind: 'cancelled' } });
		expect(
			DesktopProjectFolderAcquisitionMainEventSchema.safeParse({
				code: 'raw-electron-error',
				requestId: 'project-folder:request:3',
				type: 'desktop-project-folder-acquisition-failed',
			}).success,
		).toBe(false);
		expect(
			DesktopProjectFolderAcquisitionMainEventSchema.safeParse({
				requestId: 'project-folder:request:4',
				result: { kind: 'selected', path: 'bad\0path' },
				type: 'desktop-project-folder-acquisition-completed',
			}).success,
		).toBe(false);
	});
});
