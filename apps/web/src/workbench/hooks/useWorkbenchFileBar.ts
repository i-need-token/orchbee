import { createTypedActionRuntime, type ActionRuntime, type ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileBarMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import { loadFileBar, type FileBarModule } from '../../file-bar';
import { FileSessionRegistry, type FileSessionPort } from '../../file-session';
import { registerWorkbenchFileBarActions } from '../actions/registerWorkbenchFileBarActions';
import type { WorkbenchActionCatalog } from '../actions/workbench-action-ids';

const DEFAULT_FILE_BAR_WIDTH = 300;
const MINIMUM_FILE_BAR_WIDTH = 240;
const MAXIMUM_FILE_BAR_WIDTH = 560;

type FileBarLoad =
	| { readonly kind: 'failed' }
	| { readonly kind: 'idle' }
	| { readonly kind: 'ready'; readonly module: FileBarModule };

const IDLE_LOAD: FileBarLoad = { kind: 'idle' };

/** The File Bar docked beside the terminal area, as the Workbench lays it out. */
interface WorkbenchFileBarColumn {
	/** The loaded column, or null while its chunk loads or after it failed to. */
	readonly content: FileBarModule['FileBarColumn'] | null;
	readonly defaultSize: string;
	/** What the column says instead of its views when they could not load. */
	readonly failure: string | null;
	readonly label: string;
	readonly maxSize: string;
	readonly minSize: string;
}

export interface WorkbenchFileBarView {
	/** Present while the window offers the File Bar and it shows docked. */
	readonly column: WorkbenchFileBarColumn | null;
}

interface UseWorkbenchFileBarOptions {
	readonly actionRuntime: ActionRuntime;
	/** The Workbench's own scope, where the File Bar's Actions resolve. */
	readonly actionScope: ActionScope;
	/** The mobile layout, which has no room to dock the File Bar. */
	readonly mobile: boolean;
	/** Whether this window offers the File Bar; without the offer nothing registers or loads. */
	readonly offered: boolean;
	/** Where file sessions come from; without it the File Bar opens none. */
	readonly sessionPort: FileSessionPort | undefined;
	/** The active Workspace, whose files the File Bar shows; null outside a Workspace. */
	readonly workspaceId: string | null;
}

/**
 * The File Bar's place in the Workbench. It starts hidden; Toggle File Bar shows it, and the first
 * show loads its views through the File Bar's lazy entry. While it shows, it holds the active
 * Workspace's file session, so a session starts only for a Workspace whose files it shows.
 */
export function useWorkbenchFileBar({
	actionRuntime,
	actionScope,
	mobile,
	offered,
	sessionPort,
	workspaceId,
}: UseWorkbenchFileBarOptions): WorkbenchFileBarView {
	const intl = useOrchbeeIntl();
	const [visible, setVisible] = React.useState(false);
	const [load, setLoad] = React.useState<FileBarLoad>(IDLE_LOAD);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
		[actionRuntime],
	);
	const toggle = useMemoizedFn(() => {
		// A failed load tries again the next time the File Bar shows.
		setLoad((current) => (current.kind === 'failed' ? IDLE_LOAD : current));
		setVisible((current) => !current);
	});
	// The mobile layout has no room to dock it yet, so it offers no Action there.
	const reachable = offered && !mobile;
	React.useEffect(() => {
		if (!reachable) return undefined;
		const registration = registerWorkbenchFileBarActions({
			isVisible: visible,
			scope: actionScope,
			toggle,
			typedRuntime,
		});
		return () => registration.dispose();
	}, [actionScope, reachable, toggle, typedRuntime, visible]);
	const docked = reachable && visible;
	const sessions = React.useMemo(
		() => (sessionPort === undefined ? null : new FileSessionRegistry(sessionPort)),
		[sessionPort],
	);
	React.useEffect(() => {
		if (sessions === null || !docked || workspaceId === null) return undefined;
		const hold = sessions.hold(workspaceId);
		return () => hold.release();
	}, [docked, sessions, workspaceId]);
	React.useEffect(() => {
		if (!docked || load.kind !== 'idle') return undefined;
		let current = true;
		void loadFileBar().then(
			(module) => {
				if (current) setLoad({ kind: 'ready', module });
			},
			() => {
				if (current) setLoad({ kind: 'failed' });
			},
		);
		return () => {
			current = false;
		};
	}, [docked, load.kind]);
	if (!docked) return { column: null };
	return {
		column: {
			content: load.kind === 'ready' ? load.module.FileBarColumn : null,
			defaultSize: `${DEFAULT_FILE_BAR_WIDTH}px`,
			failure: load.kind === 'failed' ? intl.formatMessage(fileBarMessages.loadFailed) : null,
			label: intl.formatMessage(fileBarMessages.label),
			maxSize: `${MAXIMUM_FILE_BAR_WIDTH}px`,
			minSize: `${MINIMUM_FILE_BAR_WIDTH}px`,
		},
	};
}
