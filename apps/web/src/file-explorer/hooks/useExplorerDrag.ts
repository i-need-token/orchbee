import { useOrchbeeIntl } from '@orchbee/i18n';
import { useMemo } from 'react';

import { FILE_DRAG } from '../../file-drag';
import type { FilePathRules } from '../../file-session';
import type { VirtualTreeDragAndDrop } from '../../virtual-tree';
import type { ExplorerItem } from '../model/explorer-item';

/**
 * The Explorer's part in dragging its rows, while a tree with its machine's `rules` shows: a drag
 * carries the files and folders it holds, labelled as VS Code's Explorer labels one, with the entry's
 * name, or with how many entries it holds.
 */
export function useExplorerDrag(
	rules: FilePathRules | null,
): VirtualTreeDragAndDrop<string, ExplorerItem> | undefined {
	const intl = useOrchbeeIntl();
	return useMemo<VirtualTreeDragAndDrop<string, ExplorerItem> | undefined>(() => {
		if (rules === null) return undefined;
		return {
			drag: (rows) => {
				const [first] = rows;
				const label =
					rows.length === 1 && first !== undefined
						? first.element.name
						: intl.formatNumber(rows.length);
				return FILE_DRAG.carry({ paths: rows.map((row) => row.element.path), rules }, label);
			},
		};
	}, [intl, rules]);
}
