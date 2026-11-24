import * as React from 'react';

import { useTerminalSettingsMachines } from '../../hooks/useTerminalSettingsMachines';
import type { TerminalSettingsMachines } from '../../types/terminal-settings-surface';
import styles from './MachineMoveStatus.module.css';

/** How long a move's notice stays; each new notice shows once. */
const NOTICE_MILLISECONDS = 8000;

interface MachineMoveStatusProps {
	readonly machines: TerminalSettingsMachines;
}

/**
 * Change machine's status, where the TUI's status line says it: Moving… while a move runs, then
 * what a move that did not just land where it was asked to left. A move that landed adds nothing,
 * since the new shell's prompt shows where it started.
 */
export function MachineMoveStatus({ machines }: MachineMoveStatusProps): React.ReactNode {
	const move = useTerminalSettingsMachines(machines)?.move ?? null;
	const notice = move?.notice ?? null;
	const [dismissed, setDismissed] = React.useState<number | null>(null);
	const number = notice?.number ?? null;
	React.useEffect(() => {
		if (number === null) return undefined;
		const timer = window.setTimeout(() => setDismissed(number), NOTICE_MILLISECONDS);
		return () => window.clearTimeout(timer);
	}, [number]);
	const text =
		move?.pending ?? (notice !== null && notice.number !== dismissed ? notice.text : null);
	if (text === null) return null;
	return (
		<div className={styles['root']} data-slot='machine-move-status' role='status'>
			{text}
		</div>
	);
}
