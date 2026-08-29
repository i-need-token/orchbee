import { describe, expect, it } from 'vitest';

import {
	DesktopSshProjectEnrollmentMainEventSchema,
	DesktopSshProjectEnrollmentRendererCommandSchema,
} from '../desktop-ssh-project-enrollment-contract';

describe('Desktop SSH Project enrollment contract', () => {
	it('accepts only bounded user-authored SSH Project enrollment input', () => {
		const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse({
			input: {
				destination: 'ssh alice@production.example.com',
			},
			requestId: 'ssh-project:1',
			type: 'desktop-ssh-project-enrollment-requested',
		});

		expect(command).not.toHaveProperty('environmentId');
		expect(command).not.toHaveProperty('resolvedTarget');
		expect(command).not.toHaveProperty('runtimeInstanceId');
		expect(command).not.toHaveProperty('argv');
	});

	it('accepts Folder attachment with only a target Project and SSH destination', () => {
		const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse({
			input: { destination: 'alice@production.example.com' },
			projectId: 'project-1',
			requestId: 'ssh-project-folder:1',
			type: 'desktop-ssh-project-folder-attachment-requested',
		});

		expect(command).not.toHaveProperty('environmentId');
		expect(command).not.toHaveProperty('path');
		expect(command).not.toHaveProperty('directoryId');
	});

	it('rejects SSH options, control characters, and renderer authority', () => {
		for (const candidate of [
			{
				input: {
					destination: 'ssh -v',
				},
				requestId: 'ssh-project:option',
				type: 'desktop-ssh-project-enrollment-requested',
			},
			{
				input: {
					destination: 'ssh -p 2222 production',
				},
				requestId: 'ssh-project:2',
				type: 'desktop-ssh-project-enrollment-requested',
			},
			{
				environmentId: 'environment:00000000000000000000000000000000',
				input: {
					destination: 'production',
				},
				requestId: 'ssh-project:3',
				type: 'desktop-ssh-project-enrollment-requested',
			},
			{
				input: {
					destination: 'production\n-oProxyCommand=bad',
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

	it('correlates bounded directory snapshots and exact navigation revisions', () => {
		const event = DesktopSshProjectEnrollmentMainEventSchema.parse({
			requestId: 'ssh-project:directory',
			revision: 3,
			snapshot: {
				canonicalPath: '/home/alice',
				directories: [{ name: '.config' }, { name: 'src' }],
				navigationFailure: null,
				parentPath: '/home',
				truncated: false,
			},
			type: 'desktop-ssh-project-directory-selection-requested',
		});
		expect(event).toMatchObject({ requestId: 'ssh-project:directory', revision: 3 });
		expect(
			DesktopSshProjectEnrollmentRendererCommandSchema.parse({
				requestId: 'ssh-project:directory',
				result: { kind: 'navigate', path: '/srv/project' },
				revision: 3,
				type: 'desktop-ssh-project-directory-selection-resolved',
			}),
		).toMatchObject({ revision: 3 });
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

	it('publishes enrollment, Folder attachment, connection, and status without Environment authority', () => {
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
				requestId: 'ssh-project-folder:2',
				result: {
					canonicalPath: '/srv/orchbee/src',
					directoryId: 'environment-directory:00000000000000000000000000000001',
					kind: 'attached',
					projectId: 'project-1',
				},
				type: 'desktop-ssh-project-folder-attachment-settled',
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

	it('correlates reusable Environment catalog and explicit management commands', () => {
		const environmentId = `environment:${'a'.repeat(32)}`;
		for (const command of [
			{
				refreshDiscovery: true,
				requestId: 'ssh-catalog:1',
				type: 'desktop-ssh-environment-catalog-requested',
			},
			{
				input: { destination: 'production' },
				requestId: 'ssh-add:1',
				type: 'desktop-ssh-environment-add-requested',
			},
			{
				environmentId,
				requestId: 'ssh-connect:1',
				type: 'desktop-ssh-environment-connect-requested',
			},
			{
				environmentId,
				requestId: 'ssh-disconnect:1',
				type: 'desktop-ssh-environment-disconnect-requested',
			},
		]) {
			expect(DesktopSshProjectEnrollmentRendererCommandSchema.parse(command)).toEqual(command);
		}

		const catalogEvent = DesktopSshProjectEnrollmentMainEventSchema.parse({
			catalog: {
				discoveryIssue: null,
				managed: [
					{
						blockedReason: null,
						destination: 'production',
						environmentId,
						name: 'production',
						projectCount: 2,
						state: 'connected',
					},
				],
				registryIssue: null,
				suggestions: [{ destination: 'gpu-lab' }],
			},
			requestId: 'ssh-catalog:1',
			type: 'desktop-ssh-environment-catalog-published',
		});
		expect(catalogEvent).not.toHaveProperty('configurationFilePath');
		expect(JSON.stringify(catalogEvent)).not.toContain('/.ssh/config');
	});

	it('rejects pattern Host entries and privileged SSH configuration from suggestions', () => {
		const catalog = {
			discoveryIssue: null,
			managed: [],
			registryIssue: null,
			suggestions: [{ destination: '*.example.com' }],
		};
		expect(
			DesktopSshProjectEnrollmentMainEventSchema.safeParse({
				catalog,
				requestId: 'ssh-catalog:pattern',
				type: 'desktop-ssh-environment-catalog-published',
			}).success,
		).toBe(false);
		expect(
			DesktopSshProjectEnrollmentMainEventSchema.safeParse({
				catalog: {
					...catalog,
					suggestions: [
						{ configurationFilePath: '/Users/alice/.ssh/config', destination: 'gpu-lab' },
					],
				},
				requestId: 'ssh-catalog:path',
				type: 'desktop-ssh-environment-catalog-published',
			}).success,
		).toBe(false);
	});
});
