import { useEffect, useRef, useSyncExternalStore } from 'react';
import type { TabAlertState } from '../../../../../../../model/tab-model';
import {
	AlertIndicatorButtonController,
	getTabAlertColorRole,
	getTabAlertStateText,
	type AlertIndicatorSnapshot,
	type TabAlertColorRole,
} from '../model/alert-indicator-button';

interface UseAlertIndicatorButtonOptions {
	alertState: TabAlertState;
}

interface AlertIndicatorButtonView {
	visible: boolean;
	snapshot: AlertIndicatorSnapshot;
	label: string;
	colorRole: TabAlertColorRole;
}

export function useAlertIndicatorButton({
	alertState,
}: UseAlertIndicatorButtonOptions): AlertIndicatorButtonView {
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
	useEffect(() => () => controller.destroy(), [controller]);

	return {
		visible: snapshot.showingAlertState !== 'none',
		snapshot,
		label: getTabAlertStateText(snapshot.showingAlertState),
		colorRole: getTabAlertColorRole(snapshot.showingAlertState),
	};
}
