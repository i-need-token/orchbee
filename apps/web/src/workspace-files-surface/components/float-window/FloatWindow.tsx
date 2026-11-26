import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { Button, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../workbench';
import { splitPath } from '../../change-groups';
import type { EditorDocuments } from '../../EditorDocuments';
import { documentPathOf, type FloatState } from '../../float-tabs';
import type { MatchReveal } from '../../search-results';
import {
	formatFloatWindowShortcut,
	matchesFloatWindowShortcut,
} from '../../float-window-shortcuts';
import type { DiffOptions, EditorPaneHandle } from './components/editor-pane/EditorPane';
import { FloatTabStrip } from './components/float-tab-strip/FloatTabStrip';
import { UnsavedCloseDialog } from './components/unsaved-close-dialog/UnsavedCloseDialog';

import styles from './FloatWindow.module.css';

const LazyEditorPane = React.lazy(() =>
	import('./components/editor-pane/EditorPane').then((module) => ({
		default: module.EditorPane,
	})),
);

export interface FloatWindowProps {
	readonly source: WorkbenchWorkspaceFilesSource;
	readonly workspaceId: string;
	readonly state: FloatState;
	readonly refreshCount: number;
	readonly diffOptions: DiffOptions;
	/** The working-tree files the window edits, which outlive it. */
	readonly documents: EditorDocuments;
	readonly shortcutPlatform: KeyboardShortcutPlatform;
	readonly onDiffOptionsChange: (options: DiffOptions) => void;
	readonly onActivate: (id: string) => void;
	readonly onKeep: (id: string) => void;
	/** Opens a file's own tab and keeps it. */
	readonly onOpenFile: (path: string) => void;
	readonly onClose: (ids: ReadonlySet<string>) => void;
	readonly onCollapse: () => void;
	readonly onExpand: () => void;
	/** Hands the keyboard back to the terminal the window covered. */
	readonly onReturnFocus: () => void;
	/** A search match the editor selects. */
	readonly reveal: MatchReveal | null;
	/** Opens the next or previous search match, as F4 and Shift+F4 do. */
	readonly onStepSearch: (direction: 1 | -1) => void;
}

/** One of the window's icon actions, labelled for screen readers and on hover. */
function iconButton(
	label: string,
	icon: React.ComponentProps<typeof ProductIcon>['name'],
	onClick: () => void,
	pressed?: boolean,
	options?: { readonly disabled?: boolean; readonly title?: string },
): React.ReactNode {
	return (
		<Button
			aria-label={label}
			aria-pressed={pressed}
			className={styles['action']}
			disabled={options?.disabled}
			onClick={onClick}
			size='icon-xs'
			title={options?.title ?? label}
			variant='ghost'
		>
			<ProductIcon name={icon} />
		</Button>
	);
}

/**
 * The float window: files and diffs opened from the right bar, shown over the Tab's terminals
 * without resizing them. It has its own tab strip; Esc or choosing a Tab above collapses it to a
 * pill that brings it back, and its close button closes it with every file in it.
 */
export function FloatWindow({
	source,
	workspaceId,
	state,
	refreshCount,
	diffOptions,
	documents,
	shortcutPlatform,
	onDiffOptionsChange,
	onActivate,
	onKeep,
	onOpenFile,
	onClose,
	onCollapse,
	onExpand,
	onReturnFocus,
	reveal,
	onStepSearch,
}: FloatWindowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const pane = React.useRef<EditorPaneHandle | null>(null);
	const root = React.useRef<HTMLElement | null>(null);
	const active = state.tabs.find((tab) => tab.id === state.activeId) ?? state.tabs[0];
	React.useSyncExternalStore(documents.subscribe, documents.getVersion);
	const dirtyPaths = documents.dirtyPaths(workspaceId);
	const dirtyIds = new Set(
		state.tabs.flatMap((tab) => {
			const path = documentPathOf(tab.target);
			return path !== null && dirtyPaths.has(path) ? [tab.id] : [];
		}),
	);
	const activePath = active === undefined ? null : documentPathOf(active.target);
	const activeDocument = activePath === null ? null : documents.state(workspaceId, activePath);
	const canSave =
		activeDocument !== null &&
		activeDocument.editable &&
		activeDocument.dirty &&
		!activeDocument.saving;
	// Tabs whose closing waits on the person, with the files whose edits it would lose.
	const [pendingClose, setPendingClose] = React.useState<{
		readonly ids: ReadonlySet<string>;
		readonly paths: readonly string[];
	} | null>(null);

	const collapse = useMemoizedFn(() => {
		onCollapse();
		onReturnFocus();
	});
	const finishClose = useMemoizedFn((ids: ReadonlySet<string>) => {
		const closesAll = state.tabs.every((tab) => ids.has(tab.id));
		onClose(ids);
		if (closesAll) onReturnFocus();
	});
	/** Closes tabs, asking first when that loses edits no tab left open still shows. */
	const requestClose = useMemoizedFn((ids: ReadonlySet<string>) => {
		const kept = new Set(
			state.tabs.filter((tab) => !ids.has(tab.id)).map((tab) => documentPathOf(tab.target)),
		);
		const lost = [
			...new Set(
				state.tabs.flatMap((tab) => {
					const path = ids.has(tab.id) ? documentPathOf(tab.target) : null;
					return path !== null && dirtyPaths.has(path) && !kept.has(path) ? [path] : [];
				}),
			),
		];
		if (lost.length === 0) finishClose(ids);
		else setPendingClose({ ids, paths: lost });
	});
	const closeWindow = useMemoizedFn(() => {
		requestClose(new Set(state.tabs.map((tab) => tab.id)));
	});
	const saveAndClose = useMemoizedFn(async () => {
		if (pendingClose === null) return;
		const { ids, paths } = pendingClose;
		setPendingClose(null);
		const results = await Promise.all(
			paths.map((path) => documents.save(source, workspaceId, path)),
		);
		// A file typed into while it saved, or whose save did not land, keeps its tab.
		const stillDirty = documents.dirtyPaths(workspaceId);
		const unsaved = paths.filter(
			(path, index) => results[index] !== 'written' || stillDirty.has(path),
		);
		if (unsaved.length === 0) {
			finishClose(ids);
			return;
		}
		const first = state.tabs.find((tab) => {
			const path = documentPathOf(tab.target);
			return path !== null && unsaved.includes(path);
		});
		if (first !== undefined) onActivate(first.id);
	});
	const save = useMemoizedFn(() => {
		if (canSave && activePath !== null) {
			void documents.save(source, workspaceId, activePath);
		}
	});
	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLElement>) => {
		// Inside the window the key always means saving the file, never the browser saving the page.
		if (matchesFloatWindowShortcut(event.nativeEvent, 'save', shortcutPlatform)) {
			event.preventDefault();
			save();
			return;
		}
		if (event.key === 'F4' && !event.altKey && !event.ctrlKey && !event.metaKey) {
			event.preventDefault();
			onStepSearch(event.shiftKey ? -1 : 1);
			return;
		}
		// The editor takes Esc first, to close its find or suggestion widgets.
		if (event.key !== 'Escape' || event.defaultPrevented) return;
		event.preventDefault();
		event.stopPropagation();
		collapse();
	});
	// A window that opens or changes tab takes the keyboard, so Esc and the editor work at once. A
	// match opened from the search results leaves the keyboard there, and one opened to be edited
	// is focused by the editor once its text shows.
	const revealed = React.useRef(reveal?.serial ?? null);
	React.useEffect(() => {
		if (!state.open) return;
		if (reveal !== null && reveal.serial !== revealed.current) {
			revealed.current = reveal.serial;
			if (!reveal.focus || root.current?.contains(document.activeElement) === true) return;
		}
		root.current?.focus({ preventScroll: true });
	}, [state.open, state.activeId, reveal]);

	if (active === undefined) return null;
	if (!state.open) {
		return (
			<button
				className={styles['pill']}
				data-slot='workspace-float-pill'
				onClick={onExpand}
				title={intl.formatMessage(workbenchMessages.rightBarFloatReopen)}
				type='button'
			>
				<ProductIcon name='layout' />
				{intl.formatMessage(workbenchMessages.rightBarFloatPill, { count: state.tabs.length })}
			</button>
		);
	}
	const isDiff = active.target.kind === 'diff';
	const saveLabel = intl.formatMessage(workbenchMessages.rightBarFloatSave);
	const saveShortcut = formatFloatWindowShortcut('save', shortcutPlatform);
	const { name } = splitPath(
		active.target.kind === 'file' ? active.target.path : active.target.item.path,
	);
	const toggle = (key: keyof DiffOptions): void => {
		onDiffOptionsChange({ ...diffOptions, [key]: !diffOptions[key] });
	};
	return (
		<>
			<section
				aria-label={intl.formatMessage(workbenchMessages.rightBarFloatLabel, { name })}
				className={styles['window']}
				data-slot='workspace-float-window'
				onKeyDown={onKeyDown}
				ref={root}
				tabIndex={-1}
			>
				<div className={styles['header']}>
					<FloatTabStrip
						activeId={active.id}
						dirtyIds={dirtyIds}
						onActivate={onActivate}
						onClose={(id) => requestClose(new Set([id]))}
						onKeep={onKeep}
						tabs={state.tabs}
					/>
					<div className={styles['actions']}>
						{isDiff ? (
							<>
								<div
									aria-label={intl.formatMessage(workbenchMessages.rightBarDiffLayout)}
									className={styles['segments']}
									role='group'
								>
									<button
										aria-pressed={diffOptions.sideBySide}
										className={styles['segment']}
										onClick={() => onDiffOptionsChange({ ...diffOptions, sideBySide: true })}
										type='button'
									>
										{intl.formatMessage(workbenchMessages.rightBarDiffSideBySide)}
									</button>
									<button
										aria-pressed={!diffOptions.sideBySide}
										className={styles['segment']}
										onClick={() => onDiffOptionsChange({ ...diffOptions, sideBySide: false })}
										type='button'
									>
										{intl.formatMessage(workbenchMessages.rightBarDiffInline)}
									</button>
								</div>
								{iconButton(
									intl.formatMessage(workbenchMessages.rightBarDiffIgnoreWhitespace),
									'whitespace',
									() => toggle('ignoreTrimWhitespace'),
									diffOptions.ignoreTrimWhitespace,
								)}
								{iconButton(
									intl.formatMessage(workbenchMessages.rightBarDiffHideUnchanged),
									'fold',
									() => toggle('hideUnchangedRegions'),
									diffOptions.hideUnchangedRegions,
								)}
								{iconButton(
									intl.formatMessage(workbenchMessages.rightBarDiffPrevious),
									'arrow-up',
									() => pane.current?.goToChange('previous'),
								)}
								{iconButton(
									intl.formatMessage(workbenchMessages.rightBarDiffNext),
									'arrow-down',
									() => pane.current?.goToChange('next'),
								)}
								<span className={styles['separator']} />
							</>
						) : null}
						{activeDocument?.editable === true ? (
							<>
								{iconButton(saveLabel, 'save', save, undefined, {
									disabled: !canSave,
									title: activeDocument.saving
										? intl.formatMessage(workbenchMessages.rightBarFloatSaving)
										: `${saveLabel} (${saveShortcut})`,
								})}
								<span className={styles['separator']} />
							</>
						) : null}
						{iconButton(
							intl.formatMessage(workbenchMessages.rightBarFloatClose),
							'close',
							closeWindow,
						)}
					</div>
				</div>
				<React.Suspense
					fallback={
						<p className={styles['note']}>
							{intl.formatMessage(workbenchMessages.rightBarLoading)}
						</p>
					}
				>
					<LazyEditorPane
						active={active}
						diffOptions={diffOptions}
						documents={documents}
						handleRef={pane}
						onOpenFile={onOpenFile}
						refreshCount={refreshCount}
						reveal={reveal}
						source={source}
						tabs={state.tabs}
						workspaceId={workspaceId}
					/>
				</React.Suspense>
			</section>
			<UnsavedCloseDialog
				onCancel={() => setPendingClose(null)}
				onDiscard={() => {
					if (pendingClose === null) return;
					setPendingClose(null);
					finishClose(pendingClose.ids);
				}}
				onSave={() => void saveAndClose()}
				paths={pendingClose?.paths ?? null}
			/>
		</>
	);
}
