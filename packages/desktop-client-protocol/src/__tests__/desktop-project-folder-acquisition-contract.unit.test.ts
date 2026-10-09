import { describe, expect, it } from 'vitest';

import {
	DesktopNewProjectFolderNameSchema,
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
} from '../desktop-project-folder-acquisition-contract';

describe('Desktop Project Folder-acquisition contract', () => {
	it('admits one bounded name and rejects traversal, Win32 aliases, ADS and encoded length overflow', () => {
		for (const name of ['project', '项目', 'a b', '.config', 'COM10'])
			expect(DesktopNewProjectFolderNameSchema.safeParse(name).success).toBe(true);
		for (const name of [
			'',
			' ',
			'.',
			'..',
			'../x',
			'a/b',
			'a\\b',
			'C:relative',
			'C:\\absolute',
			'\\\\server\\share',
			'x:stream',
			'bad\0name',
			'bad\nname',
			'bad?',
			'bad\uD800',
			'bad\uDC00',
			'CON',
			'nul.txt',
			'COM¹.log',
			'LPT9',
			'CONOUT$',
			'con .txt',
			'trailing.',
			'trailing ',
			'trailing\u00a0',
			'a'.repeat(256),
			'项'.repeat(86),
		])
			expect(DesktopNewProjectFolderNameSchema.safeParse(name).success, name).toBe(false);
		expect(DesktopNewProjectFolderNameSchema.safeParse('a'.repeat(255)).success).toBe(true);
		expect(DesktopNewProjectFolderNameSchema.safeParse('项'.repeat(85)).success).toBe(true);
	});

	it('allows creation only through a parent token and explicit attempt, with no renderer path field', () => {
		const command = {
			attempt: 1,
			name: 'new',
			parentRequestId: 'parent:1',
			requestId: 'create:1',
			type: 'desktop-project-folder-creation-requested',
		};
		expect(DesktopProjectFolderAcquisitionRendererCommandSchema.safeParse(command).success).toBe(
			true,
		);
		expect(
			DesktopProjectFolderAcquisitionRendererCommandSchema.safeParse({
				...command,
				path: '/arbitrary',
			}).success,
		).toBe(false);
		expect(
			DesktopProjectFolderAcquisitionRendererCommandSchema.safeParse({ ...command, attempt: 0 })
				.success,
		).toBe(false);
	});
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
