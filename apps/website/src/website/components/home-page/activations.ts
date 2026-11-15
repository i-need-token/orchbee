import type { Activation } from '../../activation';
import { activateHeroStage } from './components/hero/components/hero-stage/activateHeroStage';

// The home page's sections that need script, in page order. A section's activation module lives
// beside its component and is added here.
export const homePageActivations: readonly Activation[] = [activateHeroStage];
