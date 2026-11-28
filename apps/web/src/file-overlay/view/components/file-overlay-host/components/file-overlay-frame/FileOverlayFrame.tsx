import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { useId, useRef, type ReactNode } from 'react';

import {
	useWorkbenchKeyboardScope,
	type WorkbenchKeyboardScope,
} from '../../../../../../workbench-keyboard-scopes';
import { useFileOverlayHostStore } from '../../contexts/file-overlay-host-store';
import {
	FileOverlayPip,
	type FileOverlayPipPlacement,
} from './components/file-overlay-pip/FileOverlayPip';
import { FileOverlayTitleActions } from './components/file-overlay-title-actions/FileOverlayTitleActions';
import { FileTabContent } from './components/file-tab-content/FileTabContent';
import { FileTabRow } from './components/file-tab-row/FileTabRow';
import { useFileOverlayEscape } from './hooks/useFileOverlayEscape';
import { useFileOverlayFocusReturn } from './hooks/useFileOverlayFocusReturn';
import { fileTabDomId } from './utils/fileTabDomId';
import styles from './FileOverlayFrame.module.css';

/** Focus anywhere in the frame reaches the overlay's keys, then the File Bar's. */
const FILE_OVERLAY_KEYBOARD_SCOPE: WorkbenchKeyboardScope = { kind: 'file-overlay' };

interface FileOverlayFrameProps {
	/** Where picture-in-picture shows while the overlay is shrunk to it; null while it is open. */
	readonly pip: FileOverlayPipPlacement | null;
}

/**
 * The overlay's frame: a region landmark, never a dialog, since the rest of the window stays
 * operable. Its row holds the file tabs, then the active tab's title actions, and ends with Expand,
 * or Restore while the tier is full; the tabs control the content below as its tab panel.
 * Picture-in-picture is the same frame at its own box, so the content stays mounted: the row hides,
 * and the content shows beneath picture-in-picture's click shield, out of reach of the keys. Esc that
 * nothing inside took shrinks the open overlay.
 */
export function FileOverlayFrame({ pip }: FileOverlayFrameProps): ReactNode {
	const intl = useOrchbeeIntl();
	const rootReference = useRef<HTMLElement>(null);
	const contentReference = useRef<HTMLDivElement>(null);
	const idPrefix = useId();
	const panelId = `${idPrefix}-panel`;
	useWorkbenchKeyboardScope(rootReference, FILE_OVERLAY_KEYBOARD_SCOPE);
	useFileOverlayFocusReturn(rootReference);
	const escape = useFileOverlayEscape(pip !== null);
	const activeIndex = useFileOverlayHostStore((state) =>
		state.tabs.tabs.findIndex((tab) => tab.id === state.tabs.activeId),
	);
	return (
		<section
			aria-label={intl.formatMessage(fileOverlayMessages.label)}
			className={styles['root']}
			data-pip={pip === null ? undefined : ''}
			data-slot='file-overlay-frame'
			ref={rootReference}
			// A press where nothing takes focus keeps it in the open overlay rather than letting the
			// covered terminal take it; picture-in-picture's controls take their own.
			tabIndex={pip === null ? -1 : undefined}
			onKeyDown={escape}
		>
			<div className={styles['row']} data-slot='file-overlay-row' hidden={pip !== null}>
				<FileTabRow idPrefix={idPrefix} panelId={panelId} shown={pip === null} />
				<FileOverlayTitleActions />
			</div>
			<div
				aria-labelledby={activeIndex < 0 ? undefined : fileTabDomId(idPrefix, activeIndex)}
				className={styles['content']}
				data-slot='file-overlay-content'
				id={panelId}
				inert={pip !== null}
				ref={contentReference}
				role='tabpanel'
			>
				<FileTabContent />
			</div>
			{pip === null ? null : (
				<FileOverlayPip
					box={pip.box}
					contentReference={contentReference}
					frameReference={rootReference}
					move={pip.move}
					regions={pip.regions}
				/>
			)}
		</section>
	);
}
