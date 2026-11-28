import type { ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import type { PointerDrag } from '../pointer-drag';
import { useWorkbenchKeyboardScope } from '../workbench-keyboard-scopes';

import { VirtualTreeContent } from './components/virtual-tree-content/VirtualTreeContent';
import { VirtualTreeFind } from './components/virtual-tree-find/VirtualTreeFind';
import { VirtualTreeFrame } from './components/virtual-tree-frame/VirtualTreeFrame';
import { VirtualTreeSlot } from './components/virtual-tree-slot/VirtualTreeSlot';
import { VirtualTreeStickyRows } from './components/virtual-tree-sticky-rows/VirtualTreeStickyRows';
import { useSlotCount } from './hooks/useSlotCount';
import { useTreeDrag } from './hooks/useTreeDrag';
import { useTreeFind } from './hooks/useTreeFind';
import { useTreeKeyboard } from './hooks/useTreeKeyboard';
import { useTreePointer } from './hooks/useTreePointer';
import { useViewportSize } from './hooks/useViewportSize';
import { useVirtualTreeScrollbar } from './hooks/useVirtualTreeScrollbar';
import { useWheelScrolling } from './hooks/useWheelScrolling';
import { virtualTreeKeyChords } from './interaction/tree-keys';
import type { VirtualTreeKeyboardNavigationLabel } from './interaction/type-navigation';
import { TreeViewport, type VirtualTreeRowHeight } from './layout/TreeViewport';
import type { VirtualTreeModel } from './model/VirtualTreeModel';
import type { VirtualTreeRow, VirtualTreeSegment } from './model/virtual-tree-types';
import type { VirtualTreeDragAndDrop } from './types/virtual-tree-drag';
import type {
	VirtualTreeContextMenuEvent,
	VirtualTreeOpenEvent,
} from './types/virtual-tree-events';
import type { VirtualTreeFindOptions } from './types/virtual-tree-find';
import type { VirtualTreeRowRenderer } from './types/virtual-tree-row-renderer';
import styles from './VirtualTree.module.css';

export interface VirtualTreeProps<K, T> {
	readonly model: VirtualTreeModel<K, T>;
	/** The tree's accessible name. */
	readonly label: string;
	/**
	 * Every row's height in CSS pixels, VS Code's 22 by default, or a function that reads a row's.
	 * A function is read for every row whenever it changes identity, so keep it stable.
	 */
	readonly rowHeight?: VirtualTreeRowHeight<K, T>;
	/**
	 * Empty room below the last row in CSS pixels, which the tree scrolls into, as VS Code's lists'
	 * `paddingBottom` leaves it; none unless set. VS Code's Explorer leaves one row's height.
	 */
	readonly paddingBottom?: number;
	/**
	 * How many expanded rows at most stick at the top while their descendants scroll beneath them,
	 * VS Code's 7 by default; 0 turns sticky scroll off. They never cover more than 40% of the tree.
	 */
	readonly maxStickyRows?: number;
	readonly renderRow: VirtualTreeRowRenderer<K, T>;
	/**
	 * Whether a row holds its own controls, as VS Code's SCM input and action button rows do: it
	 * draws no twistie and paints no hover or selection.
	 */
	readonly isControlRow?: ((row: VirtualTreeRow<K, T>) => boolean) | undefined;
	/** Which platform's keys the tree answers to, as VS Code's lists do on it. */
	readonly platform: KeyboardShortcutPlatform;
	/**
	 * The tree's widget in the shortcut catalog, such as `{ widget: 'explorer' }`. While the tree has
	 * focus, the bindings declared with the scope `widget:explorer` apply first, resolved in
	 * `actionScope` when given, and then the tree's own keys, which no wider binding takes. Without it
	 * the tree registers no keyboard scope, and a wider binding takes a key it shares with the tree.
	 */
	readonly keyboardScope?:
		| { readonly widget: string; readonly actionScope?: ActionScope | undefined }
		| undefined;
	/** Opens a row: on a click, a double click, a middle click, or a key that selects one row. */
	readonly onOpen?: ((event: VirtualTreeOpenEvent<K, T>) => void) | undefined;
	/**
	 * Hears a request for the context menu once the tree has focused its row. The menu itself comes
	 * from a view's `ContextMenu` trigger around the tree, which the same event opens.
	 */
	readonly onContextMenu?: ((event: VirtualTreeContextMenuEvent<K, T>) => void) | undefined;
	/**
	 * A row's text for type navigation and the find, such as a file's name; without it, typing does
	 * nothing and the tree has no find. VS Code's Explorer gives a compact row its items' names joined
	 * by `/`.
	 */
	readonly keyboardNavigationLabel?: VirtualTreeKeyboardNavigationLabel<K, T> | undefined;
	/**
	 * How the find starts, which the tree has while it has `keyboardNavigationLabel`: Mod+Alt+F or F3
	 * opens it above the rows, as VS Code's `list.find` does. A new default mode or match type applies
	 * at once, as a change to VS Code's settings does, and a provider finds rows the model has not
	 * loaded.
	 */
	readonly find?: VirtualTreeFindOptions<K, T> | undefined;
	/**
	 * Lets the pointer drag rows and drop on them, as VS Code's tree does through its drag and drop
	 * controller; without a part, the tree neither drags nor takes drops.
	 */
	readonly dragAndDrop?: VirtualTreeDragAndDrop<K, T> | undefined;
	/** Holds the tree's focusable element, for a view that moves focus to the tree. */
	readonly treeReference?: React.RefObject<HTMLDivElement | null> | undefined;
}

/**
 * A tree or list over a `VirtualTreeModel`, drawing only the rows in sight. It fills its container,
 * which must give it a definite size, scrolls by the wheel and its scrollbar as VS Code's lists do,
 * keeps the model's scroll offset, so a tree shown again opens where it was left, and keeps the rows
 * whose descendants are in sight stuck at the top, as VS Code's sticky scroll does. Keys, clicks,
 * the context menu and dragging work as in VS Code's trees, and on the items of a compact row as on
 * VS Code's compact folders, whose names `renderRow` draws with `VirtualTreeSegments`.
 */
export function VirtualTree<K, T>({
	dragAndDrop,
	find: findOptions,
	isControlRow,
	keyboardNavigationLabel,
	keyboardScope,
	label,
	maxStickyRows,
	model,
	onContextMenu,
	onOpen,
	paddingBottom,
	platform,
	renderRow,
	rowHeight,
	treeReference,
}: VirtualTreeProps<K, T>): React.ReactNode {
	const treeId = React.useId();
	const ownFrameReference = React.useRef<HTMLDivElement>(null);
	const frameReference = treeReference ?? ownFrameReference;
	const scrollbarReference = React.useRef<HTMLDivElement>(null);
	const viewport = React.useMemo(() => new TreeViewport(model), [model]);
	React.useLayoutEffect(() => {
		viewport.setRowHeight(rowHeight);
	}, [rowHeight, viewport]);
	React.useLayoutEffect(() => {
		viewport.setPaddingBottom(paddingBottom);
	}, [paddingBottom, viewport]);
	React.useLayoutEffect(() => {
		viewport.setMaxStickyRows(maxStickyRows);
	}, [maxStickyRows, viewport]);
	const slotCount = useSlotCount(viewport);
	useViewportSize(frameReference, viewport);
	useWheelScrolling(frameReference, viewport);
	useVirtualTreeScrollbar(frameReference, scrollbarReference, viewport);
	const open = useMemoizedFn((event: VirtualTreeOpenEvent<K, T>) => onOpen?.(event));
	const showContextMenu = useMemoizedFn((event: VirtualTreeContextMenuEvent<K, T>) =>
		onContextMenu?.(event),
	);
	const navigationLabel = useMemoizedFn(
		(row: VirtualTreeRow<K, T>) => keyboardNavigationLabel?.(row) ?? '',
	);
	const rowLabel = keyboardNavigationLabel === undefined ? null : navigationLabel;
	const focusTree = useMemoizedFn(() => {
		frameReference.current?.focus();
	});
	const find = useTreeFind(viewport, rowLabel, findOptions, focusTree);
	useTreeKeyboard(frameReference, viewport, platform, open, rowLabel, find);
	useTreePointer(frameReference, scrollbarReference, viewport, platform, open, showContextMenu);
	const dragItems = useMemoizedFn(
		(items: readonly VirtualTreeSegment<K, T>[]) => dragAndDrop?.drag?.(items) ?? null,
	);
	const dragOver = useMemoizedFn(
		(drag: PointerDrag, target: VirtualTreeSegment<K, T> | null) =>
			dragAndDrop?.over?.(drag, target) ?? null,
	);
	const dropOn = useMemoizedFn((drag: PointerDrag, target: VirtualTreeSegment<K, T> | null) => {
		dragAndDrop?.drop?.(drag, target);
	});
	useTreeDrag(frameReference, scrollbarReference, viewport, {
		drag: dragAndDrop?.drag === undefined ? null : dragItems,
		drop: dropOn,
		over: dragAndDrop?.over === undefined ? null : dragOver,
	});
	useWorkbenchKeyboardScope(
		frameReference,
		keyboardScope === undefined
			? null
			: {
					actionScope: keyboardScope.actionScope,
					kind: 'widget',
					widget: keyboardScope.widget,
					yields: virtualTreeKeyChords(platform, find !== null),
				},
	);

	const slots: React.ReactNode[] = [];
	for (let slot = 0; slot < slotCount; slot += 1) {
		slots.push(
			<VirtualTreeSlot
				isControlRow={isControlRow}
				key={slot}
				renderRow={renderRow}
				slot={slot}
				treeId={treeId}
				viewport={viewport}
			/>,
		);
	}
	return (
		<div className={styles['root']} data-slot='virtual-tree'>
			{find === null ? null : <VirtualTreeFind find={find} platform={platform} />}
			<VirtualTreeFrame
				label={label}
				ref={frameReference}
				scrollbarReference={scrollbarReference}
				treeId={treeId}
				viewport={viewport}
			>
				<VirtualTreeContent viewport={viewport}>
					{slots}
					<VirtualTreeSlot
						isControlRow={isControlRow}
						key='focused'
						renderRow={renderRow}
						slot='focused'
						treeId={treeId}
						viewport={viewport}
					/>
				</VirtualTreeContent>
				<VirtualTreeStickyRows renderRow={renderRow} viewport={viewport} />
			</VirtualTreeFrame>
		</div>
	);
}
