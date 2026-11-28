import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	useLayoutEffect,
	useState,
	type CSSProperties,
	type PointerEvent,
	type ReactNode,
} from 'react';

import type { PointerDropTarget } from '../../../../pointer-drag';
import { fileOverlayCover } from '../../../model';
import type { FileOverlayHostProps, FileOverlayRegionElements } from '../../file-overlay-contract';
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

/** The shown Workspace's overlay: nothing while it is closed, the scrim and frame while it is open. */
export function FileOverlayHost({
	commands,
	dispatch,
	focusTerminal,
	platform,
	ref,
	regions,
	scrimDrop,
	shortcutLabels,
	sidebarDocked,
	store,
}: FileOverlayHostProps): ReactNode {
	const focusRequest = useFileOverlayFocusRequest(ref);
	return (
		<FileOverlayHostStoreProvider
			commands={commands}
			dispatch={dispatch}
			focusRequest={focusRequest}
			focusTerminal={focusTerminal}
			platform={platform}
			shortcutLabels={shortcutLabels}
			store={store}
		>
			<FileOverlayLayer regions={regions} scrimDrop={scrimDrop} sidebarDocked={sidebarDocked} />
		</FileOverlayHostStoreProvider>
	);
}

interface FileOverlayLayerProps {
	readonly regions: FileOverlayRegionElements;
	readonly scrimDrop: PointerDropTarget | null;
	readonly sidebarDocked: boolean;
}

/**
 * The open overlay over the Workbench, outside its layout, so the terminal area keeps its size. The
 * scrim dims the tier's cover below the 36px row and the frame sits inside it; the rest of the window
 * stays operable. Files dragged over the scrim drop on the terminal it covers. While the window
 * offers left-expand, the frame's left edge chooses between it and default.
 */
function FileOverlayLayer({ regions, scrimDrop, sidebarDocked }: FileOverlayLayerProps): ReactNode {
	const open = useFileOverlayHostStore((state) => state.mode.kind === 'open');
	const tier = useFileOverlayHostStore((state) => state.mode.tier);
	const leftExpandOffered = useFileOverlayHostStore((state) => state.mode.leftExpandOffered);
	const dispatch = useFileOverlayHostDispatch();
	const focusRequest = useFileOverlayHostFocusRequest();
	const measured = useFileOverlayRegions(regions, sidebarDocked, open);
	// An ask the overlay does not open for lapses, so that it cannot move the keys when it opens later.
	// While it opens, the frame's row takes the ask once it shows.
	useLayoutEffect(() => {
		if (focusRequest.count > 0 && !open) focusRequest.take();
	}, [focusRequest, open]);
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
	if (!open || measured === null) return null;
	const cover = fileOverlayCover(measured, tier);
	const edgeTier = tier === 'full' || !leftExpandOffered || measured.sidebar === null ? null : tier;
	return (
		<div
			className={styles['layer']}
			data-slot='file-overlay'
			data-tier={tier}
			style={
				{
					'--ob-comp-file-overlay-cover-block-size': `${cover.height}px`,
					'--ob-comp-file-overlay-cover-block-start': `${cover.top}px`,
					'--ob-comp-file-overlay-cover-inline-size': `${cover.width}px`,
					'--ob-comp-file-overlay-cover-inline-start': `${cover.left}px`,
				} satisfies CSSProperties
			}
		>
			<div
				aria-hidden='true'
				className={styles['scrim']}
				data-drop-target={dropTarget || undefined}
				data-slot='file-overlay-scrim'
				onPointerDown={dismiss}
				ref={setScrim}
			/>
			<FileOverlayFrame />
			{edgeTier === null ? null : <FileOverlayEdge regions={measured} tier={edgeTier} />}
		</div>
	);
}
