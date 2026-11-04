import * as React from 'react';

import type { PanelVisualSelectionMode } from '../../../../../../model/panel-visual-selection';
import type { PanelWorkspaceMessages } from '../../../../../../types/panel-workspace-view';
import styles from './WorkbenchPanelVisualSelectionTarget.module.css';

interface WorkbenchPanelVisualSelectionTargetProps {
	readonly label: string;
	readonly messages: PanelWorkspaceMessages;
	readonly mode: PanelVisualSelectionMode;
	readonly panelId: string;
	readonly prefix: string;
}

export function WorkbenchPanelVisualSelectionTarget({
	label,
	messages,
	mode,
	panelId,
	prefix,
}: WorkbenchPanelVisualSelectionTargetProps): React.ReactNode {
	const matchingPrefix = label.startsWith(prefix) ? prefix : '';
	const remainingLabel = label.slice(matchingPrefix.length);
	return (
		<button
			aria-label={
				mode === 'focus'
					? messages.visualSelectionFocusTarget(label)
					: messages.visualSelectionSwapTarget(label)
			}
			className={styles['root']}
			data-label={label}
			data-panel-id={panelId}
			data-panel-visual-selection-target='true'
			tabIndex={-1}
			type='button'
		>
			<span aria-hidden='true' className={styles['badge']}>
				{matchingPrefix.length > 0 && <span>{matchingPrefix}</span>}
				<span className={styles['remainder']}>{remainingLabel}</span>
			</span>
		</button>
	);
}
