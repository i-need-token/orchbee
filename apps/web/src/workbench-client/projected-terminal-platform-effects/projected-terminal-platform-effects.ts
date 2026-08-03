import type { ProjectedTerminalSideEffectResult } from '@orchbee/terminal';

export interface ProjectedTerminalPlatformEffects {
	readonly openExternal?: (
		target: string,
	) => ProjectedTerminalSideEffectResult | Promise<ProjectedTerminalSideEffectResult>;
	readonly writeClipboardText?: (
		text: string,
	) => ProjectedTerminalSideEffectResult | Promise<ProjectedTerminalSideEffectResult>;
}
