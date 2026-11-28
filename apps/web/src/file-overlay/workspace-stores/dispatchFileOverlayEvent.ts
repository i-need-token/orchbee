import type { FileOverlayEvent, FileTab } from '../model';
import type { FileOverlayStore } from './FileOverlayRegistry';

/** What closing tabs asks of the content behind them; whoever shows the content answers. */
export interface FileTabContentPort {
	/**
	 * Hears the tabs a close took, in closing order, once any save question about them settled: the
	 * content lets them go, and closing a commit message tab completes its commit, as closing VS Code's
	 * commit editor does.
	 */
	readonly closed: (tabs: readonly FileTab[]) => void;
	/** Drops these tabs' edits after Don't Save, so the content matches the disk again. */
	readonly revert: (tabs: readonly FileTab[]) => void;
	/**
	 * Saves these tabs' edits after Save All and resolves with the ids of the tabs that failed, having
	 * told the person why; those tabs stay open.
	 */
	readonly save: (tabs: readonly FileTab[]) => Promise<readonly string[]>;
}

/** What running the overlay's effects needs from the Workbench around it. */
interface FileOverlayEffectPorts {
	readonly content: FileTabContentPort;
	/** Gives the current terminal the keys, as dismissing the open overlay does. */
	readonly focusTerminal: () => void;
}

/**
 * Applies an event to a Workspace's overlay and runs the effects it returns. A save's outcome returns
 * to the same overlay, whichever Workspace shows by then; a save that rejects saved nothing, so its
 * tabs stay open.
 */
export function dispatchFileOverlayEvent(
	store: FileOverlayStore,
	event: FileOverlayEvent,
	ports: FileOverlayEffectPorts,
): void {
	for (const effect of store.dispatch(event)) {
		switch (effect.kind) {
			case 'closed':
				ports.content.closed(effect.tabs);
				break;
			case 'focus-terminal':
				ports.focusTerminal();
				break;
			case 'revert':
				ports.content.revert(effect.tabs);
				break;
			case 'save': {
				const ids = effect.tabs.map((tab) => tab.id);
				void ports.content
					.save(effect.tabs)
					.catch(() => ids)
					.then((failedIds) => {
						dispatchFileOverlayEvent(store, { failedIds, type: 'close-saved' }, ports);
					});
				break;
			}
		}
	}
}
