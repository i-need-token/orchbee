import { createTypedActionRuntime, type ActionRuntime, type ActionScope } from '@orchbee/actions';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import { invokeWorkbenchClientUpdate } from '../actions/invokeWorkbenchClientUpdate';
import { registerWorkbenchClientUpdateAction } from '../actions/registerWorkbenchClientUpdateAction';
import type { WorkbenchActionCatalog } from '../actions/workbench-action-ids';
import type { WorkbenchClientUpdate } from '../types/workbench-client-update';

/** The sidebar footer's Update control while a newer version is offered. */
export interface WorkbenchClientUpdateView {
	readonly label: string;
	readonly onUpdate: () => void;
	readonly tooltip: string;
}

interface UseWorkbenchClientUpdateOptions {
	readonly clientUpdate: WorkbenchClientUpdate | undefined;
	/**
	 * Where Update is registered: the host's scope when the host's own surfaces, such as Settings'
	 * About, run it too, else the Workbench's. The footer runs it from `scope`, at or below it.
	 */
	readonly registrationScope: ActionScope;
	/** Shows that the update waits until no terminal runs, in the window's status notice. */
	readonly reportWaiting: () => void;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

const NO_OFFER = (): null => null;
const NO_SUBSCRIPTION = (): (() => void) => () => undefined;

/**
 * Update, while the host offers a newer version: its Action, registered again whenever the offer
 * changes so every surface reads its availability anew, the footer control that invokes it, and
 * the notice the host's waiting answer shows, whichever surface asked.
 */
export function useWorkbenchClientUpdate({
	clientUpdate,
	registrationScope,
	reportWaiting,
	runtime,
	scope,
}: UseWorkbenchClientUpdateOptions): WorkbenchClientUpdateView | undefined {
	const intl = useOrchbeeIntl();
	const version = React.useSyncExternalStore(
		clientUpdate?.subscribe ?? NO_SUBSCRIPTION,
		clientUpdate?.getSnapshot ?? NO_OFFER,
		clientUpdate?.getSnapshot ?? NO_OFFER,
	);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
		[runtime],
	);
	React.useEffect(() => {
		if (clientUpdate === undefined) return undefined;
		const registration = registerWorkbenchClientUpdateAction({
			clientUpdate,
			scope: registrationScope,
			typedRuntime,
		});
		return () => registration.dispose();
	}, [clientUpdate, registrationScope, typedRuntime, version]);
	React.useEffect(
		() => clientUpdate?.subscribeWaiting(reportWaiting),
		[clientUpdate, reportWaiting],
	);
	if (version === null) return undefined;
	return {
		label: intl.formatMessage(workbenchMessages.update),
		onUpdate: () => invokeWorkbenchClientUpdate(runtime, scope),
		tooltip: intl.formatMessage(workbenchMessages.updateTooltip, { version }),
	};
}
