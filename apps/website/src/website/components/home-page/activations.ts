import type { Activation } from '../../activation';
import { activateAgentRing } from './components/agents/components/agent-ring/activateAgentRing';
import { activateCollaboration } from './components/collaboration/activateCollaboration';
import { activateHeroStage } from './components/hero/components/hero-stage/activateHeroStage';
import { activateMultiplexing } from './components/multiplexing/activateMultiplexing';

// The home page's sections that need script, in page order. A section's activation module lives
// beside its component and is added here.
export const homePageActivations: readonly Activation[] = [
	activateHeroStage,
	activateMultiplexing,
	activateCollaboration,
	activateAgentRing,
];
