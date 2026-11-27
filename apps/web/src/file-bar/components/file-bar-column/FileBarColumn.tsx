import { resolveOrchbeeLocale, useOrchbeeIntl } from '@orchbee/i18n';
import { useEffect, useId, useMemo, useRef, type ReactNode } from 'react';

import { prefetchCodeEditor } from '../../../code-editor';
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

/** The column's own 36px band, where the view toggles sit, above the active view. */
export function FileBarColumn({
	activeView,
	onSelectView,
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
	const viewReference = useFileBarViewFocusRequest(viewFocusRequest);
	return (
		<div className={styles['root']} ref={rootReference}>
			<div className={styles['togglesRow']} data-slot='file-bar-toggles-row'>
				<FileBarViewSwitcher
					activeView={activeView}
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
			/>
		</div>
	);
}
