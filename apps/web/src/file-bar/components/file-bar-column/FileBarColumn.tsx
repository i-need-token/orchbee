import { resolveOrchbeeLocale, useOrchbeeIntl } from '@orchbee/i18n';
import { useEffect, useRef, type ReactNode } from 'react';

import { prefetchCodeEditor } from '../../../code-editor';
import {
	useWorkbenchKeyboardScope,
	type WorkbenchKeyboardScope,
} from '../../../workbench-keyboard-scopes';
import styles from './FileBarColumn.module.css';

/** Focus anywhere in the column reaches the File Bar's keys and its `Mod+K` sequences. */
const FILE_BAR_KEYBOARD_SCOPE: WorkbenchKeyboardScope = { kind: 'file-bar' };

/** The column's own 36px band, where the view toggles sit, above the active view. */
export function FileBarColumn(): ReactNode {
	const rootReference = useRef<HTMLDivElement>(null);
	useWorkbenchKeyboardScope(rootReference, FILE_BAR_KEYBOARD_SCOPE);
	const locale = resolveOrchbeeLocale([useOrchbeeIntl().locale]);
	// The editor loads at idle once the column shows, so the first file opened does not wait for it.
	useEffect(() => prefetchCodeEditor(locale), [locale]);
	return (
		<div className={styles['root']} ref={rootReference}>
			<div className={styles['togglesRow']} data-slot='file-bar-toggles-row' />
		</div>
	);
}
