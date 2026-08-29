import type { DesktopSshProjectDirectorySnapshot } from '@orchbee/desktop-client-protocol';
import { desktopSshProjectMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { RemoteDirectoryBrowserLocation } from '../../../remote-directory-browser';
import type { DesktopSshProjectEnrollmentDialogState } from '../model/DesktopSshProjectEnrollmentController';

type OpenDialogState = Exclude<DesktopSshProjectEnrollmentDialogState, { readonly kind: 'closed' }>;

export function desktopSshProjectDialogTitleMessage(
	state: OpenDialogState,
	choosingDirectory: boolean,
): MessageDescriptor {
	if (choosingDirectory) {
		return state.kind === 'attachment'
			? desktopSshProjectMessages.attachmentDirectoryTitle
			: desktopSshProjectMessages.directoryTitle;
	}
	if (state.kind === 'attachment') return desktopSshProjectMessages.attachmentTitle;
	if (state.kind === 'enrollment') return desktopSshProjectMessages.title;
	if (state.kind === 'environment-connection') {
		return desktopSshProjectMessages.environmentConnectionTitle;
	}
	return desktopSshProjectMessages.connectionTitle;
}

export function desktopSshProjectDialogDescriptionMessage(
	state: OpenDialogState,
	choosingDirectory: boolean,
): MessageDescriptor {
	if (choosingDirectory) {
		return state.kind === 'attachment'
			? desktopSshProjectMessages.attachmentDirectoryDescription
			: desktopSshProjectMessages.directoryDescription;
	}
	if (state.kind === 'attachment') return desktopSshProjectMessages.attachmentDescription;
	if (state.kind === 'enrollment') return desktopSshProjectMessages.description;
	if (state.kind === 'environment-connection') {
		return desktopSshProjectMessages.environmentConnectionDescription;
	}
	return desktopSshProjectMessages.connectionDescription;
}

export function desktopSshProjectProgressMessage(state: OpenDialogState): MessageDescriptor {
	switch (state.progress) {
		case 'committing-project-folder':
			return desktopSshProjectMessages.progressCommittingProjectFolder;
		case 'committing-project':
			return desktopSshProjectMessages.progressCommittingProject;
		case 'inspecting-project':
			return desktopSshProjectMessages.progressInspectingProject;
		case 'recording-environment':
			return desktopSshProjectMessages.progressRecordingEnvironment;
		case 'resolving-target':
			return desktopSshProjectMessages.progressResolvingTarget;
		case 'selecting-project-directory':
			return desktopSshProjectMessages.progressSelectingDirectory;
		case 'connecting':
		case null:
			return desktopSshProjectMessages.progressConnecting;
	}
}

export function toRemoteDirectoryLocation(
	directory: DesktopSshProjectDirectorySnapshot,
): RemoteDirectoryBrowserLocation {
	return {
		canonicalPath: directory.canonicalPath,
		crumbs: posixCrumbs(directory.canonicalPath),
		directories: directory.directories.map((entry) => ({
			name: entry.name,
			path: joinDirectory(directory.canonicalPath, entry.name),
		})),
		navigationFailure: directory.navigationFailure,
		truncated: directory.truncated,
	};
}

export function sshSourceLabel(destination: string): string {
	const trimmed = destination.trim();
	return trimmed.startsWith('ssh ') ? trimmed.slice(4) : trimmed;
}

function joinDirectory(parent: string, name: string): string {
	return parent === '/' ? `/${name}` : `${parent}/${name}`;
}

function posixCrumbs(path: string): RemoteDirectoryBrowserLocation['crumbs'] {
	const crumbs: { label: string; path: string }[] = [{ label: '/', path: '/' }];
	let currentPath = '';
	for (const segment of path.split('/').filter((part) => part.length > 0)) {
		currentPath = `${currentPath}/${segment}`;
		crumbs.push({ label: segment, path: currentPath });
	}
	return crumbs;
}
