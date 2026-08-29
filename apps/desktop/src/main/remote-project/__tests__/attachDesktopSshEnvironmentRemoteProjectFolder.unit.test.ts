import { SshProjectRootPathSchema } from '@orchbee/ssh-environment-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { DesktopClientSessionCoordinator } from '../../client-session';
import { EnvironmentIdSchema, type DesktopEnvironmentRegistry } from '../../environment';
import { attachDesktopSshEnvironmentRemoteProjectFolder } from '../attachDesktopSshEnvironmentRemoteProjectFolder';

describe('attachDesktopSshEnvironmentRemoteProjectFolder', () => {
	it('fails before remote inspection or Workbench mutation when the Environment is absent', async () => {
		const inspectProjectRoot = vi.fn<
			() => Promise<{
				readonly canonicalPath: ReturnType<typeof SshProjectRootPathSchema.parse>;
				readonly kind: 'resolved';
			}>
		>(async () => ({
			canonicalPath: SshProjectRootPathSchema.parse('/srv/orchbee'),
			kind: 'resolved',
		}));
		const mutateCanonicalDocument =
			vi.fn<
				Pick<DesktopClientSessionCoordinator, 'mutateCanonicalDocument'>['mutateCanonicalDocument']
			>();
		const registry = {
			get: vi.fn<DesktopEnvironmentRegistry['get']>(async () => ({
				kind: 'ready',
				value: null,
			})),
		};

		await expect(
			attachDesktopSshEnvironmentRemoteProjectFolder({
				allocateDirectoryToken: () => '1'.repeat(32),
				connections: {
					inspectProjectRoot,
					observation: () => ({ kind: 'inactive' }),
				},
				coordinator: { mutateCanonicalDocument },
				environmentId: EnvironmentIdSchema.parse(`environment:${'2'.repeat(32)}`),
				projectId: 'project-1',
				registry,
				requestedPath: '/srv/orchbee',
			}),
		).resolves.toEqual({ kind: 'failed', reason: 'environment-not-found' });
		expect(inspectProjectRoot).not.toHaveBeenCalled();
		expect(mutateCanonicalDocument).not.toHaveBeenCalled();
	});
});
