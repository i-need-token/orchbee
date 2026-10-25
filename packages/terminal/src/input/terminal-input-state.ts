export const TerminalCompositionPhase = {
	cancelledAwaitingEnd: 'cancelled-awaiting-end',
	eligible: 'eligible',
	idle: 'idle',
} as const;

export type TerminalCompositionPhase =
	(typeof TerminalCompositionPhase)[keyof typeof TerminalCompositionPhase];

export interface TerminalInputCorrelation {
	readonly text: string;
	readonly time: number;
}

export interface TerminalPasteCorrelation extends TerminalInputCorrelation {
	readonly source: 'beforeinput' | 'clipboard';
}

export const TerminalBeforeInputOrigin = {
	composition: 'composition',
	independent: 'independent',
} as const;

export type TerminalBeforeInputOrigin =
	(typeof TerminalBeforeInputOrigin)[keyof typeof TerminalBeforeInputOrigin];

type TerminalCompositionSettlement =
	| {
			readonly beforeInputObserved: boolean;
			readonly kind: 'awaiting-native-end';
	  }
	| { readonly kind: 'awaiting-beforeinput' };

export interface TerminalInputState<MouseSample> {
	readonly beforeInput: TerminalInputCorrelation | undefined;
	readonly composition: TerminalInputCorrelation | undefined;
	readonly compositionPhase: TerminalCompositionPhase;
	readonly compositionSettlement: TerminalCompositionSettlement | undefined;
	readonly compositionTextPublished: boolean;
	readonly key: TerminalInputCorrelation | undefined;
	readonly mouse: MouseSample | undefined;
	readonly paste: TerminalPasteCorrelation | undefined;
}

export interface TerminalInputDecision<MouseSample> {
	readonly duplicate: boolean;
	readonly state: TerminalInputState<MouseSample>;
}

export interface TerminalBeforeInputDecision<
	MouseSample,
> extends TerminalInputDecision<MouseSample> {
	readonly publishText: boolean;
}

export interface TerminalCompositionEndDecision<MouseSample> {
	readonly publishText: boolean;
	readonly state: TerminalInputState<MouseSample>;
}

export interface TerminalInputRepair<MouseSample> {
	readonly mouse: MouseSample | undefined;
	readonly state: TerminalInputState<MouseSample>;
}

export function createTerminalInputState<MouseSample>(): TerminalInputState<MouseSample> {
	return {
		beforeInput: undefined,
		composition: undefined,
		compositionPhase: TerminalCompositionPhase.idle,
		compositionSettlement: undefined,
		compositionTextPublished: false,
		key: undefined,
		mouse: undefined,
		paste: undefined,
	};
}

export function beginTerminalComposition<MouseSample>(
	state: TerminalInputState<MouseSample>,
): TerminalInputState<MouseSample> {
	if (state.compositionPhase === TerminalCompositionPhase.eligible) return state;
	return {
		...state,
		beforeInput: undefined,
		composition: undefined,
		compositionPhase: TerminalCompositionPhase.eligible,
		compositionSettlement: undefined,
		compositionTextPublished: false,
	};
}

export function supersedeCancelledTerminalComposition<MouseSample>(
	state: TerminalInputState<MouseSample>,
): TerminalInputState<MouseSample> {
	if (state.compositionPhase !== TerminalCompositionPhase.cancelledAwaitingEnd) return state;
	return {
		...state,
		compositionPhase: TerminalCompositionPhase.idle,
		compositionTextPublished: false,
	};
}

export function terminalCompositionUpdateIsEligible<MouseSample>(
	state: TerminalInputState<MouseSample>,
): boolean {
	return (
		state.compositionPhase === TerminalCompositionPhase.eligible && !state.compositionTextPublished
	);
}

export function finishTerminalComposition<MouseSample>(
	state: TerminalInputState<MouseSample>,
	text: string,
	time: number,
	deduplicationWindow: number,
): TerminalCompositionEndDecision<MouseSample> {
	if (
		state.compositionPhase === TerminalCompositionPhase.cancelledAwaitingEnd ||
		(state.compositionPhase === TerminalCompositionPhase.idle &&
			state.compositionSettlement?.kind === 'awaiting-native-end')
	) {
		const beforeInputObserved =
			state.compositionSettlement?.kind === 'awaiting-native-end' &&
			state.compositionSettlement.beforeInputObserved;
		return {
			publishText: false,
			state: {
				...state,
				beforeInput: undefined,
				compositionPhase: TerminalCompositionPhase.idle,
				compositionSettlement: beforeInputObserved ? undefined : { kind: 'awaiting-beforeinput' },
				compositionTextPublished: false,
			},
		};
	}
	if (state.compositionPhase !== TerminalCompositionPhase.eligible) {
		return { publishText: false, state };
	}
	const duplicate = isRecent(state.beforeInput, text, time, deduplicationWindow);
	return {
		publishText: text.length > 0 && !state.compositionTextPublished && !duplicate,
		state: {
			...state,
			beforeInput: duplicate ? undefined : state.beforeInput,
			composition:
				text.length > 0 && !state.compositionTextPublished && !duplicate
					? { text, time }
					: state.composition,
			compositionPhase: TerminalCompositionPhase.idle,
			compositionSettlement:
				text.length > 0 && !state.compositionTextPublished && !duplicate
					? { kind: 'awaiting-beforeinput' }
					: state.compositionSettlement,
			compositionTextPublished: false,
		},
	};
}

export function observeTerminalBeforeInput<MouseSample>(
	state: TerminalInputState<MouseSample>,
	text: string,
	time: number,
	deduplicationWindow: number,
	origin: TerminalBeforeInputOrigin = TerminalBeforeInputOrigin.independent,
): TerminalBeforeInputDecision<MouseSample> {
	if (
		origin === TerminalBeforeInputOrigin.composition &&
		state.compositionPhase !== TerminalCompositionPhase.eligible &&
		(state.compositionSettlement !== undefined ||
			state.compositionPhase === TerminalCompositionPhase.cancelledAwaitingEnd)
	) {
		return {
			duplicate: true,
			publishText: false,
			state: settleCompositionBeforeInput(state, text, time, deduplicationWindow),
		};
	}
	const progressingState =
		origin === TerminalBeforeInputOrigin.independent
			? supersedeCancelledTerminalComposition(state)
			: state;
	const keyDuplicate = isRecent(progressingState.key, text, time, deduplicationWindow);
	const afterKey = { ...progressingState, key: undefined };
	if (keyDuplicate) return { duplicate: true, publishText: false, state: afterKey };
	const compositionDuplicate = isRecent(afterKey.composition, text, time, deduplicationWindow);
	const publishText =
		!compositionDuplicate &&
		(afterKey.compositionPhase !== TerminalCompositionPhase.eligible ||
			!afterKey.compositionTextPublished);
	return {
		duplicate: compositionDuplicate,
		publishText,
		state: {
			...afterKey,
			beforeInput: publishText ? { text, time } : afterKey.beforeInput,
			composition: compositionDuplicate ? undefined : afterKey.composition,
			compositionTextPublished:
				publishText && afterKey.compositionPhase === TerminalCompositionPhase.eligible
					? true
					: afterKey.compositionTextPublished,
		},
	};
}

export function observeTerminalPaste<MouseSample>(
	state: TerminalInputState<MouseSample>,
	text: string,
	source: TerminalPasteCorrelation['source'],
	time: number,
	deduplicationWindow: number,
): TerminalInputDecision<MouseSample> {
	const previous = state.paste;
	const duplicate =
		previous !== undefined &&
		previous.source !== source &&
		isRecent(previous, text, time, deduplicationWindow);
	return {
		duplicate,
		state: duplicate ? { ...state, paste: undefined } : state,
	};
}

export function recordTerminalKeyInput<MouseSample>(
	state: TerminalInputState<MouseSample>,
	input: TerminalInputCorrelation,
): TerminalInputState<MouseSample> {
	return { ...state, key: input };
}

export function recordTerminalPaste<MouseSample>(
	state: TerminalInputState<MouseSample>,
	paste: TerminalPasteCorrelation,
): TerminalInputState<MouseSample> {
	return { ...state, paste };
}

export function recordTerminalMouseSample<MouseSample>(
	state: TerminalInputState<MouseSample>,
	mouse: MouseSample | undefined,
): TerminalInputState<MouseSample> {
	return { ...state, mouse };
}

export function repairTerminalInputState<MouseSample>(
	state: TerminalInputState<MouseSample>,
): TerminalInputRepair<MouseSample> {
	return {
		mouse: state.mouse,
		state: {
			beforeInput: undefined,
			composition: undefined,
			compositionPhase:
				state.compositionPhase === TerminalCompositionPhase.eligible
					? TerminalCompositionPhase.cancelledAwaitingEnd
					: state.compositionPhase,
			compositionSettlement:
				state.compositionPhase === TerminalCompositionPhase.eligible
					? { beforeInputObserved: false, kind: 'awaiting-native-end' }
					: state.compositionSettlement,
			compositionTextPublished: false,
			key: undefined,
			mouse: undefined,
			paste: undefined,
		},
	};
}

export function disposeTerminalInputState<MouseSample>(
	state: TerminalInputState<MouseSample>,
): TerminalInputRepair<MouseSample> {
	const repair = repairTerminalInputState(state);
	return {
		...repair,
		state: {
			...repair.state,
			compositionPhase: TerminalCompositionPhase.idle,
			compositionSettlement: undefined,
		},
	};
}

function isRecent(
	previous: TerminalInputCorrelation | undefined,
	text: string,
	time: number,
	deduplicationWindow: number,
): boolean {
	return (
		previous !== undefined && previous.text === text && time - previous.time < deduplicationWindow
	);
}

function settleCompositionBeforeInput<MouseSample>(
	state: TerminalInputState<MouseSample>,
	text: string,
	time: number,
	deduplicationWindow: number,
): TerminalInputState<MouseSample> {
	const composition = isRecent(state.composition, text, time, deduplicationWindow)
		? undefined
		: state.composition;
	if (state.compositionSettlement?.kind === 'awaiting-beforeinput') {
		return { ...state, composition, compositionSettlement: undefined };
	}
	return {
		...state,
		composition,
		compositionSettlement: {
			beforeInputObserved: true,
			kind: 'awaiting-native-end',
		},
	};
}
