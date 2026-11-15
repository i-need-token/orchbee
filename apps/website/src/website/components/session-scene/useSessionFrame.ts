import { sceneFinalState } from '../../scene/scene';
import { createSessionScene, sessionFrame, type SessionFrame } from './session';
import { useSessionStrings } from './useSessionStrings';

/** What the Session shows when its script has run: what a static page draws. */
export function useSessionFrame(): SessionFrame {
	const { strings } = useSessionStrings();
	return sessionFrame(sceneFinalState(createSessionScene()), strings);
}
