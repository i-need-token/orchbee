import type { Activation } from '../../activation';
import { activateCliOutputs } from './components/cli-outputs/activateCliOutputs';

// The command line page's sections that need script, in page order. A section's activation module
// lives beside its component and is added here.
export const cliPageActivations: readonly Activation[] = [activateCliOutputs];
