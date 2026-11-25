import type { WorkbenchUpdateOffer } from '../types/workbench-client-update';

/** Whether Update can carry out `offer` now: a Session move that already runs takes no request. */
export function isRequestableUpdateOffer(
	offer: WorkbenchUpdateOffer | null,
): offer is WorkbenchUpdateOffer {
	return offer !== null && !(offer.kind === 'session' && offer.state === 'updating');
}
