import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { FileBarPosition } from '../../file-bar';
import {
	PointerDragKind,
	registerPointerDropTarget,
	startPointerDrag,
	type PointerDragCargo,
	type PointerDropTarget,
} from '../../pointer-drag';

/** Drags of the File Bar's toggles row, which carry the side the File Bar leaves. */
const FILE_BAR_MOVE_DRAG = new PointerDragKind<FileBarPosition>();

/** The place a drop would move the File Bar to, across the Workbench root's height. */
interface WorkbenchFileBarMoveZone {
	/** The File Bar's width, as far as the terminal area reaches. */
	readonly inlineSize: number;
	/** Where it starts in the Workbench root, against the terminal area's far edge. */
	readonly inlineStart: number;
}

/** What a drag measured as it started. */
interface FileBarMoveGeometry {
	/** The terminal area's middle in the viewport: a drop past it, away from the File Bar, moves it. */
	readonly middle: number;
	readonly zone: WorkbenchFileBarMoveZone;
}

/** The layer over the Workbench that catches a drag of the toggles row while it runs. */
export interface WorkbenchFileBarMoveCatcher {
	readonly reference: React.RefObject<HTMLDivElement | null>;
	/** The place the File Bar would take, while a drop now would move it there. */
	readonly zone: WorkbenchFileBarMoveZone | null;
}

export interface WorkbenchFileBarMoveDrag {
	/** Present while a drag of the toggles row runs. */
	readonly catcher: WorkbenchFileBarMoveCatcher | null;
	/** Follows a press on the toggles row, which a drag turns into moving the File Bar. */
	readonly onRowPointerDown: (event: React.PointerEvent<HTMLElement>) => void;
}

interface UseWorkbenchFileBarMoveDragOptions {
	/** The panel group the File Bar shares with the terminal area. */
	readonly areaReference: React.RefObject<HTMLElement | null>;
	/** The docked File Bar's landmark. */
	readonly elementReference: React.RefObject<HTMLElement | null>;
	/** The File Bar's name, which the drag shows beside the pointer. */
	readonly label: string;
	/** Moves the File Bar to `target`, as Move File Bar Left or Right does. */
	readonly move: (target: FileBarPosition) => void;
	/** The side the File Bar takes. */
	readonly position: FileBarPosition;
	/** The Workbench root, which the catcher covers. */
	readonly rootReference: React.RefObject<HTMLElement | null>;
}

/**
 * Dragging the docked File Bar's toggles row past the Tab Strip's threshold, then past the middle of
 * the terminal area, shows the place the File Bar would take at its width on the area's other side,
 * and a release there moves it, as Move File Bar Left or Right does; a release elsewhere or Esc
 * leaves it. While the drag runs, a layer over the Workbench catches it, so the terminals and the file
 * overlay beneath take no part.
 */
export function useWorkbenchFileBarMoveDrag({
	areaReference,
	elementReference,
	label,
	move,
	position,
	rootReference,
}: UseWorkbenchFileBarMoveDragOptions): WorkbenchFileBarMoveDrag {
	const [geometry, setGeometry] = React.useState<FileBarMoveGeometry | null>(null);
	const [accepting, setAccepting] = React.useState(false);
	const catcherReference = React.useRef<HTMLDivElement>(null);
	const begin = useMemoizedFn((): PointerDragCargo | null => {
		const area = areaReference.current;
		const element = elementReference.current;
		const root = rootReference.current;
		if (area === null || element === null || root === null) return null;
		const areaBox = area.getBoundingClientRect();
		const barBox = element.getBoundingClientRect();
		const rootBox = root.getBoundingClientRect();
		// The terminal area runs between the File Bar and the far end of the group they share.
		const start = position === 'left' ? barBox.right : areaBox.left;
		const end = position === 'left' ? areaBox.right : barBox.left;
		const inlineSize = Math.min(barBox.width, Math.max(end - start, 0));
		setGeometry({
			middle: (start + end) / 2,
			zone: {
				inlineSize,
				inlineStart: (position === 'left' ? end - inlineSize : start) - rootBox.left,
			},
		});
		return FILE_BAR_MOVE_DRAG.carry(position, label);
	});
	const finish = useMemoizedFn(() => {
		setGeometry(null);
		setAccepting(false);
	});
	const onRowPointerDown = useMemoizedFn((event: React.PointerEvent<HTMLElement>) => {
		startPointerDrag(event.nativeEvent, event.currentTarget, begin, finish);
	});
	const middle = geometry?.middle ?? null;
	React.useLayoutEffect(() => {
		const catcher = catcherReference.current;
		if (middle === null || catcher === null) return undefined;
		const target: PointerDropTarget = {
			over(drag, point) {
				const from = FILE_BAR_MOVE_DRAG.read(drag);
				const accepts =
					from !== undefined &&
					(from === 'right' ? point.clientX < middle : point.clientX > middle);
				setAccepting(accepts);
				return accepts;
			},
			leave() {
				setAccepting(false);
			},
			drop(drag) {
				setAccepting(false);
				const from = FILE_BAR_MOVE_DRAG.read(drag);
				if (from !== undefined) move(from === 'right' ? 'left' : 'right');
			},
		};
		return registerPointerDropTarget(catcher, target);
	}, [middle, move]);
	return {
		catcher:
			geometry === null
				? null
				: { reference: catcherReference, zone: accepting ? geometry.zone : null },
		onRowPointerDown,
	};
}
