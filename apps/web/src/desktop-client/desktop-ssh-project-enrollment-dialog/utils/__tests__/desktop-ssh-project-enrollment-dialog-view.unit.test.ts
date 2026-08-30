import { desktopSshProjectMessages } from '@orchbee/i18n';
import { DesktopSshEnvironmentIdSchema } from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import type { DesktopSshProjectEnrollmentDialogState } from '../../model/DesktopSshProjectEnrollmentController';
import {
	desktopSshProjectDialogDescriptionMessage,
	desktopSshProjectDialogTitleMessage,
	desktopSshProjectProgressMessage,
	sshSourceLabel,
	toRemoteDirectoryLocation,
} from '../desktop-ssh-project-enrollment-dialog-view';

describe('desktop SSH Project enrollment dialog view mapping', () => {
	it('selects exact title, description, and progress descriptors', () => {
		const enrollment = enrollmentState();
		expect(desktopSshProjectDialogTitleMessage(enrollment, false)).toBe(
			desktopSshProjectMessages.title,
		);
		expect(desktopSshProjectDialogDescriptionMessage(enrollment, false)).toBe(
			desktopSshProjectMessages.description,
		);
		expect(desktopSshProjectDialogTitleMessage(enrollment, true)).toBe(
			desktopSshProjectMessages.directoryTitle,
		);
		expect(desktopSshProjectProgressMessage(enrollment)).toBe(
			desktopSshProjectMessages.progressSelectingDirectory,
		);
		const hostPrompt = {
			...enrollment,
			phase: 'prompt',
			prompt: {
				generation: 1,
				handle: 'host-prompt',
				kind: 'confirm',
				prompt: 'Verify host fingerprint?',
			},
		} as const;
		expect(desktopSshProjectDialogTitleMessage(hostPrompt, false)).toBe(
			desktopSshProjectMessages.promptHostTitle,
		);
		expect(desktopSshProjectDialogDescriptionMessage(hostPrompt, false)).toBe(
			desktopSshProjectMessages.promptHostDescription,
		);
		const authenticationPrompt = {
			...hostPrompt,
			prompt: { ...hostPrompt.prompt, kind: 'none' },
		} as const;
		expect(desktopSshProjectDialogTitleMessage(authenticationPrompt, false)).toBe(
			desktopSshProjectMessages.promptAuthenticationTitle,
		);
		expect(desktopSshProjectDialogDescriptionMessage(authenticationPrompt, false)).toBe(
			desktopSshProjectMessages.promptAuthenticationDescription,
		);

		const connection = connectionState();
		expect(desktopSshProjectDialogTitleMessage(connection, false)).toBe(
			desktopSshProjectMessages.connectionTitle,
		);
		expect(desktopSshProjectDialogDescriptionMessage(connection, false)).toBe(
			desktopSshProjectMessages.connectionDescription,
		);
		const failedConnection = { ...connection, failure: 'connection', phase: 'failed' } as const;
		expect(desktopSshProjectDialogTitleMessage(failedConnection, false)).toBe(
			desktopSshProjectMessages.failureTitle,
		);
		expect(desktopSshProjectDialogDescriptionMessage(failedConnection, false)).toBe(
			desktopSshProjectMessages.failureConnection,
		);

		const environmentConnection = environmentConnectionState();
		expect(desktopSshProjectDialogTitleMessage(environmentConnection, false)).toBe(
			desktopSshProjectMessages.environmentConnectionTitle,
		);
		expect(desktopSshProjectDialogDescriptionMessage(environmentConnection, false)).toBe(
			desktopSshProjectMessages.environmentConnectionDescription,
		);
	});

	it('adapts one canonical POSIX snapshot without inventing paths', () => {
		expect(
			toRemoteDirectoryLocation({
				canonicalPath: '/home/builder',
				directories: [{ name: '.config' }, { name: 'projects' }],
				navigationFailure: null,
				parentPath: '/home',
				truncated: false,
			}),
		).toEqual({
			canonicalPath: '/home/builder',
			crumbs: [
				{ label: '/', path: '/' },
				{ label: 'home', path: '/home' },
				{ label: 'builder', path: '/home/builder' },
			],
			directories: [
				{ name: '.config', path: '/home/builder/.config' },
				{ name: 'projects', path: '/home/builder/projects' },
			],
			navigationFailure: null,
			truncated: false,
		});
		expect(sshSourceLabel(' ssh builder@build-host ')).toBe('builder@build-host');
		expect(sshSourceLabel('build-host')).toBe('build-host');
	});
});

function enrollmentState(): Extract<
	DesktopSshProjectEnrollmentDialogState,
	{ readonly kind: 'enrollment' }
> {
	return {
		canNavigate: false,
		canSubmit: true,
		directory: null,
		failure: null,
		form: { destination: 'build-host' },
		kind: 'enrollment',
		pathInput: '',
		phase: 'working',
		progress: 'selecting-project-directory',
		prompt: null,
	};
}

function connectionState(): Extract<
	DesktopSshProjectEnrollmentDialogState,
	{ readonly kind: 'connection' }
> {
	return {
		canNavigate: false,
		directory: null,
		failure: null,
		kind: 'connection',
		pathInput: '',
		phase: 'working',
		progress: null,
		projectId: 'project-remote',
		prompt: null,
	};
}

function environmentConnectionState(): Extract<
	DesktopSshProjectEnrollmentDialogState,
	{ readonly kind: 'environment-connection' }
> {
	return {
		canNavigate: false,
		directory: null,
		environmentId: DesktopSshEnvironmentIdSchema.parse(`environment:${'a'.repeat(32)}`),
		environmentName: 'gpu-lab',
		failure: null,
		kind: 'environment-connection',
		pathInput: '',
		phase: 'working',
		progress: null,
		prompt: null,
	};
}
