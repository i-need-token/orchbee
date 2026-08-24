import type { TerminalPresentationId } from '../../../../terminal-presentation';
import type {
	WorkbenchTerminalPresentationMode,
	WorkbenchTerminalPresentationReadiness,
} from '../../../types/workbench-terminal-surface';

export interface TerminalTabPresentationLayer {
	readonly epoch: number;
	readonly mode: WorkbenchTerminalPresentationMode;
	reportPresentationReady(
		presentationId: TerminalPresentationId,
		readiness: WorkbenchTerminalPresentationReadiness,
	): void;
}
