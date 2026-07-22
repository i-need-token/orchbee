import type { ReactNode } from 'react';
import type { GlicControlsController } from '../../../../glic';

import { useGlicControls } from './hooks/useGlicControls';

export interface GlicControlsProps {
	controller: GlicControlsController;
	onGlicClick?: () => void;
	onActorClick?: () => void;
	onUnpin?: () => void;
	/** When supplied, a change hides and resets a visible Gemini nudge. */
	activeTabId?: string | null;
	className?: string;
}

export function GlicControls(props: GlicControlsProps): ReactNode {
	return useGlicControls(props);
}
