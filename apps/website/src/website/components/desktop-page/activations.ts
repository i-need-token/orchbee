import type { Activation } from '../../activation';
import { activateDesktopRendering } from './components/desktop-rendering/activateDesktopRendering';
import { activateDesktopStage } from './components/desktop-stage/activateDesktopStage';

// The desktop application page's sections that need script, in page order. A section's activation
// module lives beside its component and is added here.
export const desktopPageActivations: readonly Activation[] = [
	activateDesktopStage,
	activateDesktopRendering,
];
