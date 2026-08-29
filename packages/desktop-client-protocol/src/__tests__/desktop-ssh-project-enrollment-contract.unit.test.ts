import { describe, expect, it } from 'vitest';

import {
	DesktopSshProjectEnrollmentMainEventSchema,
	DesktopSshProjectEnrollmentRendererCommandSchema,
} from '../desktop-ssh-project-enrollment-contract';

describe('Desktop SSH Project enrollment contract', () => {
	it('accepts only bounded user-authored SSH Project enrollment input', () => {
		const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse({
			input: {
				configurationFilePath: '/Users/test/.ssh/config',
				destination: 'production',
				name: 'Production',
				projectPath: '/srv/orchbee',
			},
			requestId: 'ssh-project:1',
			type: 'desktop-ssh-project-enrollment-requested',
		});

		expect(command).not.toHaveProperty('environmentId');
		expect(command).not.toHaveProperty('resolvedTarget');
		expect(command).not.toHaveProperty('runtimeInstanceId');
		expect(command).not.toHaveProperty('argv');
	});

	it('rejects non-absolute paths, control characters, and renderer authority', () => {
		for (const candidate of [
			{
				input: {
					configurationFilePath: '.ssh/config',
					destination: 'production',
					name: 'Production',
					projectPath: '/srv/orchbee',
				},
				requestId: 'ssh-project:2',
				type: 'desktop-ssh-project-enrollment-requested',
			},
			{
				environmentId: 'environment:00000000000000000000000000000000',
				input: {
					configurationFilePath: null,
					destination: 'production',
					name: 'Production',
					projectPath: '/srv/orchbee',
				},
				requestId: 'ssh-project:3',
				type: 'desktop-ssh-project-enrollment-requested',
			},
			{
				input: {
					configurationFilePath: null,
					destination: 'production\n-oProxyCommand=bad',
					name: 'Production',
					projectPath: '/srv/orchbee',
				},
				requestId: 'ssh-project:4',
				type: 'desktop-ssh-project-enrollment-requested',
			},
		]) {
			expect(DesktopSshProjectEnrollmentRendererCommandSchema.safeParse(candidate).success).toBe(
				false,
			);
		}
	});

	it('correlates a bounded prompt to an exact request, generation, and handle', () => {
		expect(
			DesktopSshProjectEnrollmentMainEventSchema.parse({
				generation: 2,
				handle: 'ssh-prompt-0123456789abcdef',
				kind: 'none',
				prompt: 'Enter passphrase for key:',
				requestId: 'ssh-project:5',
				type: 'desktop-ssh-project-interaction-requested',
			}),
		).toMatchObject({ generation: 2, requestId: 'ssh-project:5' });
		expect(
			DesktopSshProjectEnrollmentRendererCommandSchema.safeParse({
				generation: 1,
				handle: 'ssh-prompt-0123456789abcdef',
				requestId: 'ssh-project:5',
				result: { kind: 'submitted', response: 'secret\nextra' },
				type: 'desktop-ssh-project-interaction-resolved',
			}).success,
		).toBe(false);
	});

	it('publishes enrollment, connection, and status without Environment authority', () => {
		for (const event of [
			{
				requestId: 'ssh-project:6',
				result: {
					canonicalPath: '/srv/orchbee',
					kind: 'created',
					projectId: 'project-1',
					workspaceId: 'workspace-1',
				},
				type: 'desktop-ssh-project-enrollment-settled',
			},
			{
				requestId: 'ssh-project:7',
				result: { kind: 'connected', projectId: 'project-1' },
				type: 'desktop-ssh-project-connection-settled',
			},
			{
				requestId: null,
				statuses: [{ blockedReason: null, projectId: 'project-1', state: 'disconnected' }],
				type: 'desktop-ssh-project-status-published',
			},
		]) {
			const parsed = DesktopSshProjectEnrollmentMainEventSchema.parse(event);
			expect(parsed).not.toHaveProperty('environmentId');
		}
	});
});
