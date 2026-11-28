import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	useLayoutEffect,
	useState,
	type CSSProperties,
	type PointerEvent,
	type ReactNode,
} from 'react';

import type { PointerDropTarget } from '../../../../pointer-drag';
import { fileOverlayCover, fileOverlayPipBox, type FileOverlayPipPosition } from '../../../model';
import type { FileOverlayHostProps, FileOverlayRegionElements } from '../../file-overlay-contract';
import { FileOverlayCloseDialog } from './components/file-overlay-close-dialog/FileOverlayCloseDialog';
import { FileOverlayEdge } from './components/file-overlay-edge/FileOverlayEdge';
import { FileOverlayFrame } from './components/file-overlay-frame/FileOverlayFrame';
import {
	FileOverlayHostStoreProvider,
	useFileOverlayHostDispatch,
	useFileOverlayHostFocusRequest,
	useFileOverlayHostStore,
} from './contexts/file-overlay-host-store';
import { useFileOverlayFocusRequest } from './hooks/useFileOverlayFocusRequest';
import { useFileOverlayRegions } from './hooks/useFileOverlayRegions';
import { useFileOverlayScrimDrop } from './hooks/useFileOverlayScrimDrop';
import styles from './FileOverlayHost.module.css';

/**
 * The shown Workspace's overlay: nothing while it is closed, the scrim and frame while it is open,
 * and the frame alone at picture-in-picture's box while it is shrunk; over all of them, the save
 * question while a close waits on it.
 */
export function FileOverlayHost({
	commands,
	contents,
	dispatch,
	fileBarPosition,
	focusTerminal,
	movePip,
	pipPosition,
	platform,
	ref,
	regions,
	returnFocus,
	scrimDrop,
	shortcutLabels,
	sidebarDocked,
	store,
}: FileOverlayHostProps): ReactNode {
	const focusRequest = useFileOverlayFocusRequest(ref);
	return (
		<FileOverlayHostStoreProvider
			commands={commands}
			contents={contents}
			dispatch={dispatch}
			focusRequest={focusRequest}
			focusTerminal={focusTerminal}
			platform={platform}
			returnFocus={returnFocus}
			shortcutLabels={shortcutLabels}
			store={store}
		>
			<FileOverlayLayer
				fileBarPosition={fileBarPosition}
				movePip={movePip}
				pipPosition={pipPosition}
				regions={regions}
				scrimDrop={scrimDrop}
				sidebarDocked={sidebarDocked}
			/>
			<FileOverlayCloseDialog />
		</FileOverlayHostStoreProvider>
	);
}

interface FileOverlayLayerProps {
	readonly fileBarPosition: FileOverlayHostProps['fileBarPosition'];
	readonly movePip: (position: FileOverlayPipPosition) => void;
	readonly pipPosition: FileOverlayPipPosition | null;
	readonly regions: FileOverlayRegionElements;
	readonly scrimDrop: PointerDropTarget | null;
	readonly sidebarDocked: boolean;
}

/**
 * The overlay over the Workbench, outside its layout, so the terminal area keeps its size. While it
 * is open, the scrim dims the tier's cover below the 36px row and the frame sits inside it; the rest
 * of the window stays operable. Files dragged over the scrim drop on the terminal it covers. While
 * the window offers left-expand, the frame's left edge chooses between it and default. Shrunk, the
 * same frame shows at picture-in-picture's box with everything around it operable, so its content
 * stays mounted.
 */
function FileOverlayLayer({
	fileBarPosition,
	movePip,
	pipPosition,
	regions,
	scrimDrop,
	sidebarDocked,
}: FileOverlayLayerProps): ReactNode {
	const kind = useFileOverlayHostStore((state) => state.mode.kind);
	const tier = useFileOverlayHostStore((state) => state.mode.tier);
	const leftExpandOffered = useFileOverlayHostStore((state) => state.mode.leftExpandOffered);
	const dispatch = useFileOverlayHostDispatch();
	const focusRequest = useFileOverlayHostFocusRequest();
	const shown = kind !== 'closed';
	const measured = useFileOverlayRegions(regions, sidebarDocked, fileBarPosition, shown);
	// An ask the overlay does not show for lapses, so that it cannot move the keys when it shows
	// later. While the overlay shows, the frame's row or picture-in-picture takes the ask.
	useLayoutEffect(() => {
		if (focusRequest.count > 0 && !shown) focusRequest.take();
	}, [focusRequest, shown]);
	const [scrim, setScrim] = useState<HTMLDivElement | null>(null);
	const dropTarget = useFileOverlayScrimDrop(scrimDrop, scrim);
	// A press on the scrim shrinks the overlay to picture-in-picture and gives the terminal the keys.
	// The scrim is gone before the press's mousedown, which would land on the Workbench root and blur
	// the terminal again, so the press is cancelled.
	const dismiss = useMemoizedFn((event: PointerEvent<HTMLDivElement>) => {
		if (event.button !== 0) return;
		event.preventDefault();
		dispatch({ type: 'dismissed' });
	});
	if (!shown || measured === null) return null;
	const cover = fileOverlayCover(measured, tier);
	const pipBox = fileOverlayPipBox(measured, pipPosition);
	const pip = kind === 'pip' ? { box: pipBox, move: movePip, regions: measured } : null;
	const edgeTier =
		pip !== null || tier === 'full' || !leftExpandOffered || measured.sidebar === null
			? null
			: tier;
	// Each child keeps its place whichever mode shows, so the frame and its content stay mounted.
	return (
		<div
			className={styles['layer']}
			data-mode={kind}
			data-slot='file-overlay'
			data-tier={tier}
			style={
				{
					'--ob-comp-file-overlay-cover-block-size': `${cover.height}px`,
					'--ob-comp-file-overlay-cover-block-start': `${cover.top}px`,
					'--ob-comp-file-overlay-cover-inline-size': `${cover.width}px`,
					'--ob-comp-file-overlay-cover-inline-start': `${cover.left}px`,
					'--ob-comp-file-overlay-pip-block-size': `${pipBox.height}px`,
					'--ob-comp-file-overlay-pip-block-start': `${pipBox.top}px`,
					'--ob-comp-file-overlay-pip-inline-size': `${pipBox.width}px`,
					'--ob-comp-file-overlay-pip-inline-start': `${pipBox.left}px`,
				} satisfies CSSProperties
			}
		>
			{pip === null ? (
				<div
					aria-hidden='true'
					className={styles['scrim']}
					data-drop-target={dropTarget || undefined}
					data-slot='file-overlay-scrim'
					onPointerDown={dismiss}
					ref={setScrim}
				/>
			) : null}
			<FileOverlayFrame pip={pip} />
			{edgeTier === null ? null : <FileOverlayEdge regions={measured} tier={edgeTier} />}
		</div>
	);
}
