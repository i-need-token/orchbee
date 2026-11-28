import { useMemoizedFn } from '@orchbee/foundation/react';
import { resolveOrchbeeLocale, useOrchbeeIntl } from '@orchbee/i18n';
import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
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
import { FileBarRowMenu } from './components/file-bar-row-menu/FileBarRowMenu';
import { FileBarViewSwitcher } from './components/file-bar-view-switcher/FileBarViewSwitcher';
import { useFileBarRowMenu } from './hooks/useFileBarRowMenu';
import { useFileBarViewFocusRequest } from './hooks/useFileBarViewFocusRequest';
import styles from './FileBarColumn.module.css';

/** Focus anywhere in the column reaches the File Bar's keys and its `Mod+K` sequences. */
const FILE_BAR_KEYBOARD_SCOPE: WorkbenchKeyboardScope = { kind: 'file-bar' };

/**
 * The column's own 36px band, where the view toggles sit and whose menu holds them and the File
 * Bar's place, above the active view. A view mounts the first time it shows and stays while the
 * column does, as VS Code keeps a view it has created, so switching back finds it as it was.
 */
export function FileBarColumn({
	actionRuntime,
	actionScope,
	activeFile,
	activeView,
	atWindowEnd,
	leadingPlace,
	notifications,
	onSelectView,
	openFile,
	platform,
	quickInput,
	reportActionOutcome,
	rowMenu,
	session,
	settings,
	shortcutLabelForAction,
	storage,
	toggles,
	viewFocusRequest,
}: FileBarColumnProps): ReactNode {
	const rootReference = useRef<HTMLDivElement>(null);
	const rowReference = useRef<HTMLDivElement>(null);
	const menu = useFileBarRowMenu(rowReference, rowMenu);
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
			<ContextMenu open={menu.open} onOpenChange={menu.onOpenChange}>
				<ContextMenuTrigger
					className={styles['togglesRow']}
					data-leading-place={leadingPlace === null ? undefined : ''}
					data-slot='file-bar-toggles-row'
					ref={rowReference}
				>
					{leadingPlace}
					<FileBarViewSwitcher
						activeView={activeView}
						changes={changes ?? 0}
						onSelectView={onSelectView}
						tabIdOf={tabIdOf}
						toggles={toggles}
						viewId={viewId}
					/>
					{/* The File Bar toggle floats over its place, and the window's caption buttons over theirs. */}
					{atWindowEnd ? (
						<>
							<span className={styles['fileBarTogglePlace']} data-file-bar-toggle-place='' />
							<span
								className={styles['windowControlsPlace']}
								data-slot='file-bar-window-controls-place'
							/>
						</>
					) : null}
				</ContextMenuTrigger>
				<FileBarRowMenu
					entries={menu.entries}
					finalFocus={menu.finalFocus}
					onChoose={menu.choose}
				/>
			</ContextMenu>
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
							openFile={openFile}
							platform={platform}
							ref={explorerReference}
							reportActionOutcome={reportActionOutcome}
							session={session}
							shortcutLabelForAction={shortcutLabelForAction}
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
							settings={settings}
							storage={storage}
						/>
					</div>
				) : null}
			</div>
		</div>
	);
}
