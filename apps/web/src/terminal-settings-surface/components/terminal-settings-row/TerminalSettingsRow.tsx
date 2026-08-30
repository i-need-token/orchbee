import * as React from 'react';

import styles from './TerminalSettingsRow.module.css';

interface TerminalSettingsRowProps {
	readonly children: React.ReactNode;
	readonly description: string;
	readonly label: string;
	readonly settingId: string;
	readonly status?: React.ReactNode;
	readonly visible: boolean;
}

export function TerminalSettingsRow({
	children,
	description,
	label,
	settingId,
	status,
	visible,
}: TerminalSettingsRowProps): React.ReactNode {
	if (!visible) return null;
	return (
		<div
			className={styles['root']}
			data-setting-id={settingId}
			data-slot='terminal-settings-row'
			id={settingId}
			tabIndex={-1}
		>
			<div className={styles['copy']}>
				<div className={styles['labelLine']}>
					<div className={styles['label']}>{label}</div>
					{status}
				</div>
				<div className={styles['description']}>{description}</div>
			</div>
			<div className={styles['control']}>{children}</div>
		</div>
	);
}
