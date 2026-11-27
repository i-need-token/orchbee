import type { Activation } from '../../activation';
import { activateTerminalThemes } from './components/terminal-themes/activateTerminalThemes';

// The terminal interface page's sections that need script, in page order. A section's activation
// module lives beside its component and is added here.
export const terminalPageActivations: readonly Activation[] = [activateTerminalThemes];
