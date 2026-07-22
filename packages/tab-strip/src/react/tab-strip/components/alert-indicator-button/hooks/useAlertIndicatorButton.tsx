import {
	useEffect,
	useRef,
	useSyncExternalStore,
	type CSSProperties,
	type MouseEvent,
	type PointerEvent,
	type ReactNode,
} from 'react';
import type { TabAlertState } from '../../../../../model';
import {
	AlertIndicatorButtonController,
	getTabAlertColorRole,
	getTabAlertStateText,
	isAudioAlertState,
} from '../../../../../tab';
import { TabAlertIndicatorGlyph } from '../../tab-alert-indicator-glyph/TabAlertIndicatorGlyph';
interface UseAlertIndicatorButtonOptions {
	alertState: TabAlertState;
	active: boolean;
	largestSelectableRegionWidth: number;
	enableTabMuting?: boolean;
	onToggleAudioMute?: (muted: boolean) => void;
}
export function useAlertIndicatorButton({
	alertState,
	active,
	largestSelectableRegionWidth,
	enableTabMuting = true,
	onToggleAudioMute,
}: UseAlertIndicatorButtonOptions): ReactNode {
	const controllerRef = useRef<AlertIndicatorButtonController | null>(null);
	// Chromium constructs the indicator without an alert, then applies TabData.
	// Starting empty preserves the first fade/pulse instead of treating an
	// initially supplied state as already settled.
	if (controllerRef.current === null) controllerRef.current = new AlertIndicatorButtonController();
	const controller = controllerRef.current;
	const snapshot = useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	useEffect(() => {
		controller.transitionTo(alertState);
	}, [alertState, controller]);
	useEffect(() => {
		controller.updateMuteEnvironment({
			enableTabMuting,
			active,
			indicatorWidth: 16,
			largestSelectableRegionWidth,
		});
	}, [active, controller, enableTabMuting, largestSelectableRegionWidth]);
	useEffect(() => () => controller.destroy(), [controller]);
	if (snapshot.showingAlertState === 'none') return null;
	const audio = isAudioAlertState(snapshot.showingAlertState);
	let label = getTabAlertStateText(snapshot.showingAlertState);
	if (audio) label = 'Unmute tab audio';
	if (snapshot.showingAlertState === 'audio-playing') label = 'Mute tab audio';
	return (
		<button
			type='button'
			className='ct-tab__alert'
			aria-label={label}
			disabled={!snapshot.enabledForMuteToggle}
			tabIndex={snapshot.enabledForMuteToggle ? 0 : -1}
			data-fade-kind={snapshot.fadeKind}
			data-alert-state={snapshot.showingAlertState}
			data-alert-color={getTabAlertColorRole(snapshot.showingAlertState)}
			style={
				{
					'--ct-alert-fade-duration': `${snapshot.fadeDurationMs}ms`,
				} satisfies CSSProperties & Record<'--ct-alert-fade-duration', string>
			}
			onPointerDown={(event: PointerEvent<HTMLButtonElement>) => {
				if (
					snapshot.enabledForMuteToggle &&
					event.button === 0 &&
					!event.shiftKey &&
					!event.ctrlKey
				)
					event.stopPropagation();
			}}
			onClick={(event: MouseEvent<HTMLButtonElement>) => {
				if (!snapshot.enabledForMuteToggle) return;
				event.stopPropagation();
				controller.toggleAudioMute(onToggleAudioMute);
			}}
		>
			<TabAlertIndicatorGlyph state={snapshot.showingAlertState} />
		</button>
	);
}
