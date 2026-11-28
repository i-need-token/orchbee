import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { useId, useRef, type ReactNode } from 'react';

import {
	useWorkbenchKeyboardScope,
	type WorkbenchKeyboardScope,
} from '../../../../../../workbench-keyboard-scopes';
import { useFileOverlayHostStore } from '../../contexts/file-overlay-host-store';
import { FileOverlayTitleActions } from './components/file-overlay-title-actions/FileOverlayTitleActions';
import { FileTabRow } from './components/file-tab-row/FileTabRow';
import { useFileOverlayFocusReturn } from './hooks/useFileOverlayFocusReturn';
import { fileTabDomId } from './utils/fileTabDomId';
import styles from './FileOverlayFrame.module.css';

/** Focus anywhere in the frame reaches the overlay's keys, then the File Bar's. */
const FILE_OVERLAY_KEYBOARD_SCOPE: WorkbenchKeyboardScope = { kind: 'file-overlay' };

/**
 * The overlay's frame: a region landmark, never a dialog, since the rest of the window stays
 * operable. Its row holds the file tabs, then the active tab's title actions, and ends with Expand,
 * or Restore while the tier is full; the tabs control the content below as its tab panel.
 */
export function FileOverlayFrame(): ReactNode {
	const intl = useOrchbeeIntl();
	const rootReference = useRef<HTMLElement>(null);
	const idPrefix = useId();
	const panelId = `${idPrefix}-panel`;
	useWorkbenchKeyboardScope(rootReference, FILE_OVERLAY_KEYBOARD_SCOPE);
	useFileOverlayFocusReturn(rootReference);
	const activeIndex = useFileOverlayHostStore((state) =>
		state.tabs.tabs.findIndex((tab) => tab.id === state.tabs.activeId),
	);
	return (
		<section
			aria-label={intl.formatMessage(fileOverlayMessages.label)}
			className={styles['root']}
			data-slot='file-overlay-frame'
			ref={rootReference}
		>
			<div className={styles['row']} data-slot='file-overlay-row'>
				<FileTabRow idPrefix={idPrefix} panelId={panelId} />
				<FileOverlayTitleActions />
			</div>
			<div
				aria-labelledby={activeIndex < 0 ? undefined : fileTabDomId(idPrefix, activeIndex)}
				className={styles['content']}
				data-slot='file-overlay-content'
				id={panelId}
				role='tabpanel'
			/>
		</section>
	);
}
