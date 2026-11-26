import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { Button, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../workbench';
import { splitPath } from '../../change-groups';
import type { FloatState } from '../../float-tabs';
import type { DiffOptions, EditorPaneHandle } from './components/editor-pane/EditorPane';
import { FloatTabStrip } from './components/float-tab-strip/FloatTabStrip';

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
	readonly onDiffOptionsChange: (options: DiffOptions) => void;
	readonly onActivate: (id: string) => void;
	readonly onKeep: (id: string) => void;
	readonly onClose: (ids: ReadonlySet<string>) => void;
	readonly onCollapse: () => void;
	readonly onExpand: () => void;
	/** Hands the keyboard back to the terminal the window covered. */
	readonly onReturnFocus: () => void;
}

/** One of the window's icon actions, labelled for screen readers and on hover. */
function iconButton(
	label: string,
	icon: React.ComponentProps<typeof ProductIcon>['name'],
	onClick: () => void,
	pressed?: boolean,
): React.ReactNode {
	return (
		<Button
			aria-label={label}
			aria-pressed={pressed}
			className={styles['action']}
			onClick={onClick}
			size='icon-xs'
			title={label}
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
	onDiffOptionsChange,
	onActivate,
	onKeep,
	onClose,
	onCollapse,
	onExpand,
	onReturnFocus,
}: FloatWindowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const pane = React.useRef<EditorPaneHandle | null>(null);
	const root = React.useRef<HTMLElement | null>(null);
	const active = state.tabs.find((tab) => tab.id === state.activeId) ?? state.tabs[0];
	const collapse = useMemoizedFn(() => {
		onCollapse();
		onReturnFocus();
	});
	const closeWindow = useMemoizedFn(() => {
		onClose(new Set(state.tabs.map((tab) => tab.id)));
		onReturnFocus();
	});
	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLElement>) => {
		// The editor takes Esc first, to close its find or suggestion widgets.
		if (event.key !== 'Escape' || event.defaultPrevented) return;
		event.preventDefault();
		event.stopPropagation();
		collapse();
	});
	// A window that opens or changes tab takes the keyboard, so Esc and the editor work at once.
	React.useEffect(() => {
		if (state.open) root.current?.focus({ preventScroll: true });
	}, [state.open, state.activeId]);

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
	const { name } = splitPath(
		active.target.kind === 'file' ? active.target.path : active.target.item.path,
	);
	const toggle = (key: keyof DiffOptions): void => {
		onDiffOptionsChange({ ...diffOptions, [key]: !diffOptions[key] });
	};
	return (
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
					onActivate={onActivate}
					onClose={(id) => onClose(new Set([id]))}
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
					{iconButton(
						intl.formatMessage(workbenchMessages.rightBarFloatClose),
						'close',
						closeWindow,
					)}
				</div>
			</div>
			<React.Suspense
				fallback={
					<p className={styles['note']}>{intl.formatMessage(workbenchMessages.rightBarLoading)}</p>
				}
			>
				<LazyEditorPane
					active={active}
					diffOptions={diffOptions}
					handleRef={pane}
					refreshCount={refreshCount}
					source={source}
					tabs={state.tabs}
					workspaceId={workspaceId}
				/>
			</React.Suspense>
		</section>
	);
}
