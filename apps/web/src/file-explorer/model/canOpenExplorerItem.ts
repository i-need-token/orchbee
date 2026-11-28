import type { ExplorerItem } from './explorer-item';

/**
 * Whether the file overlay can open an item: a file or a link to one, whose name a request can
 * carry. A folder only expands, and anything else opens nothing.
 */
export function canOpenExplorerItem(item: ExplorerItem): boolean {
	return item.entry?.type === 'file' && !item.entry.unrepresentable;
}
