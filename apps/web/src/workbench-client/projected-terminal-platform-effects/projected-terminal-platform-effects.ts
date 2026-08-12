import type { TerminalBell } from '@orchbee/terminal-protocol';
import type { ProjectedTerminalSideEffectResult } from '@orchbee/terminal';

export interface ProjectedTerminalPlatformEffects {
	readonly ringBell?: (event: TerminalBell) => void;
	readonly openExternal?: (
		target: string,
	) => ProjectedTerminalSideEffectResult | Promise<ProjectedTerminalSideEffectResult>;
	readonly writeClipboardText?: (
		text: string,
	) => ProjectedTerminalSideEffectResult | Promise<ProjectedTerminalSideEffectResult>;
}
