import { createTypedActionRuntime, type ActionRuntime, type ActionScope } from '@orchbee/actions';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import { registerWorkbenchClientUpdateAction } from '../actions/registerWorkbenchClientUpdateAction';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';
import type { WorkbenchClientUpdate } from '../types/workbench-client-update';

/** The sidebar footer's Update control while a newer version is offered. */
export interface WorkbenchClientUpdateView {
	readonly label: string;
	readonly onUpdate: () => void;
	readonly tooltip: string;
}

interface UseWorkbenchClientUpdateOptions {
	readonly clientUpdate: WorkbenchClientUpdate | undefined;
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
 * the notice the host's waiting answer shows.
 */
export function useWorkbenchClientUpdate({
	clientUpdate,
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
		const registration = registerWorkbenchClientUpdateAction({ clientUpdate, scope, typedRuntime });
		return () => registration.dispose();
	}, [clientUpdate, scope, typedRuntime, version]);
	React.useEffect(
		() => clientUpdate?.subscribeWaiting(reportWaiting),
		[clientUpdate, reportWaiting],
	);
	if (version === null) return undefined;
	return {
		label: intl.formatMessage(workbenchMessages.update),
		onUpdate: () => {
			typedRuntime.execute(WORKBENCH_ACTION_IDS.updateClient, undefined, {
				scope,
				source: 'button',
			});
		},
		tooltip: intl.formatMessage(workbenchMessages.updateTooltip, { version }),
	};
}
