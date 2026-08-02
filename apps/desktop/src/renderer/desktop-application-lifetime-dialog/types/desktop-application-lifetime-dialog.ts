import type { DesktopApplicationExitChoice } from '../../../shared';
import type { DesktopApplicationLifetimeState } from '../model/DesktopApplicationLifetimeCoordinator';

export interface DesktopApplicationLifetimeDialogController {
	choose(choice: DesktopApplicationExitChoice): void;
	getSnapshot(): DesktopApplicationLifetimeState;
	quitAnyway(): void;
	retry(): void;
	subscribe(listener: () => void): () => void;
}
