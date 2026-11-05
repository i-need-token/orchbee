import type { WorkbenchAuthority } from '../workbench';

function unexpected(): Promise<never> {
	return Promise.reject(new Error('The test did not expect this authority call.'));
}

/** An authority that rejects every call the test does not provide. */
export function inertWorkbenchAuthority(
	overrides: Partial<WorkbenchAuthority> = {},
): WorkbenchAuthority {
	return {
		change: unexpected,
		close: unexpected,
		create: unexpected,
		createPanelTerminal: unexpected,
		createTab: unexpected,
		duplicateTabs: unexpected,
		minimumSplitPercent: 10,
		stoppedTabClose: () => null,
		watchClose: () => () => undefined,
		...overrides,
	};
}
