import type { TerminalSettingsMachine } from '../../../../types/terminal-settings-surface';

/** The tone of a machine's state word, as the TUI's machine list colors it. */
type MachineTone = 'pending' | 'positive' | 'quiet' | 'warning';

export function machineTone(machine: TerminalSettingsMachine): MachineTone {
	if (machine.stopping) return 'pending';
	switch (machine.state) {
		case 'online':
			return 'positive';
		case 'connecting':
		case 'reconnecting':
			return 'pending';
		case 'offline':
		case 'disabled':
		case 'needs-login':
			return 'quiet';
		default:
			return 'warning';
	}
}
