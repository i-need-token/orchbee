import type { DesktopApplicationExitChoice } from '@orchbee/desktop-client-protocol';
import type { DesktopApplicationLifetimeState } from '../model/DesktopApplicationLifetimeCoordinator';

export interface DesktopApplicationLifetimeDialogController {
	choose(choice: DesktopApplicationExitChoice): void;
	getSnapshot(): DesktopApplicationLifetimeState;
	quitAnyway(): void;
	retry(): void;
	subscribe(listener: () => void): () => void;
}
