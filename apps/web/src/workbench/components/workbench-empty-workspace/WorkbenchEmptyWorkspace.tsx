import { Kbd } from '@orchbee/ui/components';
import type * as React from 'react';

import styles from './WorkbenchEmptyWorkspace.module.css';

interface WorkbenchEmptyWorkspaceProps {
	readonly fact: string;
	readonly newTabLabel: string | null;
	readonly newTabShortcutLabel: string | undefined;
}

/**
 * Sits where a terminal's first row would, as the TUI's guidance does. The hint only teaches the
 * shortcut: the Tab Strip's new-tab control, or a double-click on this content, is the pointer's way
 * to start a Tab, so the content region gains no centered creation control.
 */
export function WorkbenchEmptyWorkspace({
	fact,
	newTabLabel,
	newTabShortcutLabel,
}: WorkbenchEmptyWorkspaceProps): React.ReactNode {
	return (
		<div className={styles['root']} data-slot='workbench-empty-workspace'>
			<p className={styles['fact']}>{fact}</p>
			{newTabLabel === null || newTabShortcutLabel === undefined ? null : (
				<p className={styles['hint']}>
					<Kbd>{newTabShortcutLabel}</Kbd>
					{newTabLabel}
				</p>
			)}
		</div>
	);
}
