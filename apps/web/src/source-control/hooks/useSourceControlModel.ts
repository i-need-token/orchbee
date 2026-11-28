import { useMemoizedFn } from '@orchbee/foundation/react';
import { resolveOrchbeeLocale, sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { FileWorkspaceSession } from '../../file-session';
import type { ScmResourceStatus } from '../model/scm-resource';
import {
	SourceControlModel,
	type ScmViewMemory,
	type SourceControlState,
} from '../model/SourceControlModel';
import type { SourceControlViewStateStorage } from '../model/source-control-view-state-storage';

/** Workspaces whose Source Control memory stays while the view shows, least recently shown going. */
const REMEMBERED_WORKSPACES = 50;

const STATUS_MESSAGES = {
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

function subscribeToNothing(): () => void {
	return () => undefined;
}

interface SourceControlModelOptions {
	/** The active Workspace's file session; null while there is none to show. */
	readonly session: FileWorkspaceSession | null;
	readonly storage: SourceControlViewStateStorage | undefined;
}

interface SourceControlModelView {
	/** The model over the session; null without a session and before the first one is built. */
	readonly model: SourceControlModel | null;
	readonly state: SourceControlState | null;
}

/**
 * Source Control over the session it is given. Another session or locale builds another model, and
 * the one it replaces is disposed, ending its subscriptions and reads; what each Workspace's view
 * remembers for the run, its tree's place and the repositories closed here, stays for its next one.
 */
export function useSourceControlModel({
	session,
	storage,
}: SourceControlModelOptions): SourceControlModelView {
	const intl = useOrchbeeIntl();
	const locale = resolveOrchbeeLocale([intl.locale]);
	const [memories] = React.useState(() => new Map<string, ScmViewMemory>());
	const [model, setModel] = React.useState<SourceControlModel | null>(null);
	const statusText = useMemoizedFn((status: ScmResourceStatus) =>
		intl.formatMessage(STATUS_MESSAGES[status]),
	);
	React.useEffect(() => {
		if (session === null) return undefined;
		const memory = memories.get(session.workspaceId) ?? { changes: null, closed: new Set() };
		memories.delete(session.workspaceId);
		memories.set(session.workspaceId, memory);
		for (const workspaceId of memories.keys()) {
			if (memories.size <= REMEMBERED_WORKSPACES) break;
			memories.delete(workspaceId);
		}
		// VS Code's `compareFileNames`: numeric, case- and accent-insensitive, then by code unit.
		const collator = new Intl.Collator(locale, { numeric: true, sensitivity: 'base' });
		const built = new SourceControlModel({
			compareNames: (one, other) => {
				const result = collator.compare(one, other);
				if (result !== 0 || one === other) return result;
				return one < other ? -1 : 1;
			},
			memory,
			session,
			statusText,
			storage,
		});
		setModel(built);
		return () => {
			setModel(null);
			built.dispose();
		};
	}, [locale, memories, session, statusText, storage]);
	const state = React.useSyncExternalStore(
		model === null ? subscribeToNothing : model.subscribe,
		() => model?.state ?? null,
	);
	return { model, state };
}
