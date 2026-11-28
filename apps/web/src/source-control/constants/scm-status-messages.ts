import { sourceControlMessages } from '@orchbee/i18n';

import type { ScmResourceStatus } from '../model/scm-resource';

/** Each status's hover text, as VS Code's Git extension names it on a row and on a decoration. */
export const SCM_STATUS_MESSAGES = {
	'added-by-them': sourceControlMessages.statusAddedByThem,
	'added-by-us': sourceControlMessages.statusAddedByUs,
	'both-added': sourceControlMessages.statusBothAdded,
	'both-deleted': sourceControlMessages.statusBothDeleted,
	'both-modified': sourceControlMessages.statusBothModified,
	deleted: sourceControlMessages.statusDeleted,
	'deleted-by-them': sourceControlMessages.statusDeletedByThem,
	'deleted-by-us': sourceControlMessages.statusDeletedByUs,
	ignored: sourceControlMessages.statusIgnored,
	'index-added': sourceControlMessages.statusIndexAdded,
	'index-copied': sourceControlMessages.statusIndexCopied,
	'index-deleted': sourceControlMessages.statusIndexDeleted,
	'index-modified': sourceControlMessages.statusIndexModified,
	'index-renamed': sourceControlMessages.statusIndexRenamed,
	'intent-to-add': sourceControlMessages.statusIntentToAdd,
	'intent-to-rename': sourceControlMessages.statusIntentToRename,
	modified: sourceControlMessages.statusModified,
	'type-changed': sourceControlMessages.statusTypeChanged,
	untracked: sourceControlMessages.statusUntracked,
} as const satisfies Record<ScmResourceStatus, unknown>;
