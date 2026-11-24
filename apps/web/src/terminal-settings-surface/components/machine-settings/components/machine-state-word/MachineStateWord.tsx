import * as React from 'react';

import type { TerminalSettingsMachine } from '../../../../types/terminal-settings-surface';
import { machineTone } from './machineTone';
import styles from './MachineStateWord.module.css';

/** A machine's state word in the tone the TUI's machine list gives it. */
export function MachineStateWord({
	machine,
}: {
	readonly machine: TerminalSettingsMachine;
}): React.ReactNode {
	return (
		<span className={styles['root']} data-tone={machineTone(machine)}>
			{machine.word}
		</span>
	);
}
