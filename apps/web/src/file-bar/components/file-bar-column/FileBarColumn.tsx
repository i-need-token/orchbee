import { useMemoizedFn } from '@orchbee/foundation/react';
import { resolveOrchbeeLocale, useOrchbeeIntl } from '@orchbee/i18n';
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from 'react';

import { prefetchCodeEditor } from '../../../code-editor';
import { FileExplorer, type FileExplorerFocus } from '../../../file-explorer';
import {
	SourceControl,
	useSourceControlCount,
	type SourceControlFocus,
} from '../../../source-control';
import {
	useWorkbenchKeyboardScope,
	type WorkbenchKeyboardScope,
} from '../../../workbench-keyboard-scopes';
import type { FileBarColumnProps } from '../../file-bar-contract';
import type { FileBarViewId } from '../../file-bar-layout';
import { FileBarViewSwitcher } from './components/file-bar-view-switcher/FileBarViewSwitcher';
import { useFileBarViewFocusRequest } from './hooks/useFileBarViewFocusRequest';
import styles from './FileBarColumn.module.css';

/** Focus anywhere in the column reaches the File Bar's keys and its `Mod+K` sequences. */
const FILE_BAR_KEYBOARD_SCOPE: WorkbenchKeyboardScope = { kind: 'file-bar' };

/**
 * The column's own 36px band, where the view toggles sit, above the active view. A view mounts the
 * first time it shows and stays while the column does, as VS Code keeps a view it has created, so
 * switching back finds it as it was.
 */
export function FileBarColumn({
	actionRuntime,
	actionScope,
	activeFile,
	activeView,
	notifications,
	onSelectView,
	platform,
	quickInput,
	session,
	storage,
	viewFocusRequest,
}: FileBarColumnProps): ReactNode {
	const rootReference = useRef<HTMLDivElement>(null);
	useWorkbenchKeyboardScope(rootReference, FILE_BAR_KEYBOARD_SCOPE);
	const locale = resolveOrchbeeLocale([useOrchbeeIntl().locale]);
	// The editor loads at idle once the column shows, so the first file opened does not wait for it.
	useEffect(() => prefetchCodeEditor(locale), [locale]);
	const idPrefix = useId();
	const tabIdOf = useMemo(
		() =>
			(view: FileBarViewId): string =>
				`${idPrefix}-tab-${view}`,
		[idPrefix],
	);
	const viewId = `${idPrefix}-view`;
	const [explorerMounted, setExplorerMounted] = useState(activeView === 'explorer');
	if (activeView === 'explorer' && !explorerMounted) setExplorerMounted(true);
	const [sourceControlMounted, setSourceControlMounted] = useState(activeView === 'source-control');
	if (activeView === 'source-control' && !sourceControlMounted) setSourceControlMounted(true);
	const explorerReference = useRef<FileExplorerFocus>(null);
	const sourceControlReference = useRef<SourceControlFocus>(null);
	const focusView = useMemoizedFn((): boolean => {
		const view =
			activeView === 'explorer' ? explorerReference.current : sourceControlReference.current;
		view?.focus();
		return view !== null;
	});
	const changes = useSourceControlCount(session?.git ?? null);
	const viewReference = useFileBarViewFocusRequest(viewFocusRequest, focusView);
	return (
		<div className={styles['root']} ref={rootReference}>
			<div className={styles['togglesRow']} data-slot='file-bar-toggles-row'>
				<FileBarViewSwitcher
					activeView={activeView}
					changes={changes ?? 0}
					onSelectView={onSelectView}
					tabIdOf={tabIdOf}
					viewId={viewId}
				/>
				{/* The File Bar toggle floats over its place, and the window's caption buttons over theirs. */}
				<span className={styles['fileBarTogglePlace']} data-file-bar-toggle-place='' />
				<span
					className={styles['windowControlsPlace']}
					data-slot='file-bar-window-controls-place'
				/>
			</div>
			<div
				aria-labelledby={tabIdOf(activeView)}
				className={styles['view']}
				data-slot='file-bar-view'
				data-view={activeView}
				id={viewId}
				ref={viewReference}
				role='tabpanel'
				tabIndex={-1}
			>
				{explorerMounted ? (
					<div className={styles['viewContent']} hidden={activeView !== 'explorer'}>
						<FileExplorer
							actionRuntime={actionRuntime}
							actionScope={actionScope}
							activeFile={activeFile}
							notifications={notifications}
							platform={platform}
							ref={explorerReference}
							session={session}
							storage={storage}
						/>
					</div>
				) : null}
				{sourceControlMounted ? (
					<div className={styles['viewContent']} hidden={activeView !== 'source-control'}>
						<SourceControl
							actionRuntime={actionRuntime}
							actionScope={actionScope}
							notifications={notifications}
							platform={platform}
							quickInput={quickInput}
							ref={sourceControlReference}
							session={session}
							storage={storage}
						/>
					</div>
				) : null}
			</div>
		</div>
	);
}
