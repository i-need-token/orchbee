import { WORKBENCH_ACTION_IDS } from './workbench-action-ids';

/**
 * Invalidates every void-input composite while another composite discovers its
 * concrete descendant Action. This prevents availability checks from recursively
 * rediscovering one another while leaving the descendant input structurally exact.
 */
export const WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS = Object.freeze({
	[WORKBENCH_ACTION_IDS.moveActiveTabBackward]: null,
	[WORKBENCH_ACTION_IDS.moveActiveTabForward]: null,
	[WORKBENCH_ACTION_IDS.openPanelBroadcastInput]: null,
	[WORKBENCH_ACTION_IDS.selectNextTab]: null,
	[WORKBENCH_ACTION_IDS.selectPreviousTab]: null,
});
