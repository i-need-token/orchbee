import type { ActionConfirmation } from '@orchbee/actions';
import { sourceControlMessages } from '@orchbee/i18n';

import type { FilePathRules } from '../../file-session';
import type { ScmResource, ScmResourceStatus } from './scm-resource';

/** A change of one repository as it stands now. */
export interface ScmCurrentChange {
	readonly repository: string;
	readonly resource: ScmResource;
}

type FileOs = FilePathRules['os'];

/** Untracked and ignored files have no committed content, so discarding them deletes them. */
export function isScmUntracked(status: ScmResourceStatus): boolean {
	return status === 'untracked' || status === 'ignored';
}

function nameOf(path: string, separator: string): string {
	return path.slice(path.lastIndexOf(separator) + 1);
}

/**
 * VS Code's question before Discard Changes or Discard All Changes, as its `_cleanAll` asks it:
 * tracked files alone ask to discard, or to restore when every one is deleted; untracked files alone
 * ask to move them to the trash of the machine holding them; both together ask once, with the
 * trash's words before the tracked files' warning. VS Code's mixed question also offers to discard
 * only the tracked files, which a confirmation's single answer leaves to selecting them instead.
 */
export function scmDiscardQuestion(
	changes: readonly ScmCurrentChange[],
	os: FileOs,
	separator: string,
): ActionConfirmation {
	const tracked = changes.filter(({ resource }) => !isScmUntracked(resource.status));
	const untracked = changes.filter(({ resource }) => isScmUntracked(resource.status));
	const firstName = (group: readonly ScmCurrentChange[]): string =>
		nameOf(group[0]?.resource.path ?? '', separator);
	if (untracked.length === 0) {
		const values = { count: tracked.length, name: firstName(tracked) };
		if (tracked.every(({ resource }) => resource.status === 'deleted')) {
			return {
				confirmLabel: { ...sourceControlMessages.restoreConfirm, values },
				description: { ...sourceControlMessages.restoreDescription, values },
				risk: 'routine',
				title: { ...sourceControlMessages.restoreTitle, values },
			};
		}
		return {
			confirmLabel: { ...sourceControlMessages.discardTrackedConfirm, values },
			description: { ...sourceControlMessages.discardTrackedDescription, values },
			risk: 'destructive',
			title: { ...sourceControlMessages.discardTrackedTitle, values },
		};
	}
	const untrackedValues = { count: untracked.length, name: firstName(untracked), os };
	if (tracked.length === 0) {
		return {
			confirmLabel: { ...sourceControlMessages.deleteUntrackedConfirm, values: untrackedValues },
			description: {
				...sourceControlMessages.deleteUntrackedDescription,
				values: untrackedValues,
			},
			risk: 'elevated',
			title: { ...sourceControlMessages.deleteUntrackedTitle, values: untrackedValues },
		};
	}
	return {
		confirmLabel: {
			...sourceControlMessages.discardTrackedConfirm,
			values: { count: changes.length },
		},
		description: {
			...sourceControlMessages.discardMixedDescription,
			values: {
				name: firstName(tracked),
				os,
				tracked: tracked.length,
				untracked: untracked.length,
			},
		},
		risk: 'destructive',
		title: { ...sourceControlMessages.deleteUntrackedTitle, values: untrackedValues },
	};
}

/**
 * VS Code Git's question when the trash refuses untracked files: delete them permanently instead.
 * VS Code asks it after the first refusal and deletes every file; here it names only the files the
 * trash did not take.
 */
export function scmPermanentDeleteQuestion(count: number, os: FileOs): ActionConfirmation {
	return {
		confirmLabel: { ...sourceControlMessages.permanentConfirm, values: { count } },
		description: sourceControlMessages.permanentDescription,
		risk: 'destructive',
		title: { ...sourceControlMessages.permanentTitle, values: { os } },
	};
}
