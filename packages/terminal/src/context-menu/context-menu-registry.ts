import type { IDisposable } from '../event-emitter';
import type { TerminalInteractionContext } from '../host';
import type { ContextMenuItem } from '../terminal-contracts';

export interface ContextMenuProvider {
	provide(context: TerminalInteractionContext): readonly ContextMenuItem[];
}

/** Ordered serializable contributions for application-owned context menus. */
export class ContextMenuRegistry implements IDisposable {
	private readonly providers = new Set<ContextMenuProvider>();

	register(provider: ContextMenuProvider): IDisposable {
		this.providers.add(provider);
		return { dispose: () => this.providers.delete(provider) };
	}

	provide(
		context: TerminalInteractionContext,
		baseItems: readonly ContextMenuItem[] = [],
	): readonly ContextMenuItem[] {
		const items = [...baseItems];
		for (const provider of this.providers) items.push(...provider.provide(context));
		return insertSorted(items);
	}

	dispose(): void {
		this.providers.clear();
	}
}

function compareMenuItems(left: ContextMenuItem, right: ContextMenuItem): number {
	const groupOrder = left.group.localeCompare(right.group);
	return groupOrder === 0 ? left.order - right.order : groupOrder;
}

function insertSorted(items: readonly ContextMenuItem[]): readonly ContextMenuItem[] {
	const result: ContextMenuItem[] = [];
	for (const item of items) {
		const index = result.findIndex((existing) => compareMenuItems(item, existing) < 0);
		if (index < 0) result.push(item);
		else result.splice(index, 0, item);
	}
	return result;
}
