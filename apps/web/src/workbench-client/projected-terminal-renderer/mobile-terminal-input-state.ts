import type { ProjectedTerminalInjectedInputReadiness } from '@orchbee/terminal';

export type MobileTerminalModifier = 'alt' | 'control';

export interface MobileTerminalInputState {
	readonly alt: boolean;
	readonly control: boolean;
	readonly focused: boolean;
	readonly readiness: ProjectedTerminalInjectedInputReadiness;
}

export interface MobileTerminalInputSnapshot {
	readonly alt: boolean;
	readonly control: boolean;
	readonly revision: string;
}

export interface MobileTerminalInputConsumption {
	readonly snapshot: MobileTerminalInputSnapshot | null;
	readonly state: MobileTerminalInputState;
}

export function createMobileTerminalInputState(
	readiness: ProjectedTerminalInjectedInputReadiness = { kind: 'unavailable' },
	focused = false,
): MobileTerminalInputState {
	return { alt: false, control: false, focused, readiness };
}

export function replaceMobileTerminalInputReadiness(
	state: MobileTerminalInputState,
	readiness: ProjectedTerminalInjectedInputReadiness,
): MobileTerminalInputState {
	if (sameReadiness(state.readiness, readiness)) return state;
	return { ...state, alt: false, control: false, readiness };
}

export function setMobileTerminalInputFocus(
	state: MobileTerminalInputState,
	focused: boolean,
): MobileTerminalInputState {
	if (state.focused === focused && (focused || (!state.alt && !state.control))) return state;
	return focused
		? { ...state, focused: true }
		: { ...state, alt: false, control: false, focused: false };
}

export function toggleMobileTerminalModifier(
	state: MobileTerminalInputState,
	modifier: MobileTerminalModifier,
): MobileTerminalInputState {
	if (state.readiness.kind !== 'ready') return clearMobileTerminalModifiers(state);
	return { ...state, [modifier]: !state[modifier] };
}

export function clearMobileTerminalModifiers(
	state: MobileTerminalInputState,
): MobileTerminalInputState {
	return state.alt || state.control ? { ...state, alt: false, control: false } : state;
}

export function consumeMobileTerminalInput(
	state: MobileTerminalInputState,
): MobileTerminalInputConsumption {
	const next = clearMobileTerminalModifiers(state);
	return {
		snapshot:
			state.readiness.kind === 'ready'
				? {
						alt: state.alt,
						control: state.control,
						revision: state.readiness.revision,
					}
				: null,
		state: next,
	};
}

function sameReadiness(
	left: ProjectedTerminalInjectedInputReadiness,
	right: ProjectedTerminalInjectedInputReadiness,
): boolean {
	return (
		(left.kind === 'unavailable' && right.kind === 'unavailable') ||
		(left.kind === 'ready' && right.kind === 'ready' && left.revision === right.revision)
	);
}
