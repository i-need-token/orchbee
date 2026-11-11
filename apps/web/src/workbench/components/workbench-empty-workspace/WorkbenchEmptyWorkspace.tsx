import { Kbd } from '@orchbee/ui/components';
import type * as React from 'react';

import styles from './WorkbenchEmptyWorkspace.module.css';

interface WorkbenchEmptyWorkspaceProps {
	readonly fact: string;
	readonly newTabLabel: string | null;
	readonly newTabShortcutLabel: string | undefined;
	readonly startAgentLabel: string | null;
	readonly startAgentShortcutLabel: string | undefined;
}

/**
 * Sits where a terminal's first row would, as the TUI's guidance does. The hints only teach the
 * shortcuts for New tab and Start Agent: the Tab Strip's new-tab control and its menu, or a
 * double-click on this content, are the pointer's ways in, so the content region gains no centered
 * creation control.
 */
export function WorkbenchEmptyWorkspace({
	fact,
	newTabLabel,
	newTabShortcutLabel,
	startAgentLabel,
	startAgentShortcutLabel,
}: WorkbenchEmptyWorkspaceProps): React.ReactNode {
	const hints = [
		{ key: 'new-tab', label: newTabLabel, shortcut: newTabShortcutLabel },
		{ key: 'start-agent', label: startAgentLabel, shortcut: startAgentShortcutLabel },
	].flatMap(({ key, label, shortcut }) =>
		label === null || shortcut === undefined ? [] : [{ key, label, shortcut }],
	);
	return (
		<div className={styles['root']} data-slot='workbench-empty-workspace'>
			<p className={styles['fact']}>{fact}</p>
			{hints.length === 0 ? null : (
				// One column of keys, so the hints' words start together.
				<div className={styles['hints']}>
					{hints.map((hint) => (
						<p className={styles['hint']} data-hint={hint.key} key={hint.key}>
							<Kbd className={styles['key']}>{hint.shortcut}</Kbd>
							{hint.label}
						</p>
					))}
				</div>
			)}
		</div>
	);
}
