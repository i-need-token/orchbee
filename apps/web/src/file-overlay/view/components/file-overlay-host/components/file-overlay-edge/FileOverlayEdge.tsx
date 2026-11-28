import type { CSSProperties, ReactNode } from 'react';

import {
	fileOverlayCover,
	type FileOverlayNarrowTier,
	type FileOverlayRegions,
} from '../../../../../model';
import { useFileOverlayHostDispatch } from '../../contexts/file-overlay-host-store';
import { useFileOverlayEdgeDrag } from './hooks/useFileOverlayEdgeDrag';
import styles from './FileOverlayEdge.module.css';

interface FileOverlayEdgeProps {
	/** What the overlay lies over, which places the preview and the Sidebar's midline. */
	readonly regions: FileOverlayRegions;
	/** The open overlay's tier. */
	readonly tier: FileOverlayNarrowTier;
}

/**
 * The frame's left edge while the window offers left-expand. A drag never resizes the frame: a
 * preview frame shows the tier the release chooses, left-expand once the pointer crosses the
 * Sidebar's midline and default once it crosses back, and Esc leaves the tier as it was.
 */
export function FileOverlayEdge({ regions, tier }: FileOverlayEdgeProps): ReactNode {
	const dispatch = useFileOverlayHostDispatch();
	const { onPointerDown, preview } = useFileOverlayEdgeDrag({
		choose: (chosen) => {
			if (chosen !== tier) dispatch({ tier: chosen, type: 'tier-chosen' });
		},
		regions,
		tier,
	});
	const previewCover = preview === null ? null : fileOverlayCover(regions, preview);
	return (
		<>
			<div
				aria-hidden='true'
				className={styles['edge']}
				data-held={preview === null ? undefined : ''}
				data-slot='file-overlay-edge'
				onPointerDown={onPointerDown}
			/>
			{previewCover === null ? null : (
				<>
					<div aria-hidden='true' className={styles['shield']} />
					<div
						aria-hidden='true'
						className={styles['preview']}
						data-slot='file-overlay-edge-preview'
						style={
							{
								'--ob-comp-file-overlay-cover-block-size': `${previewCover.height}px`,
								'--ob-comp-file-overlay-cover-block-start': `${previewCover.top}px`,
								'--ob-comp-file-overlay-cover-inline-size': `${previewCover.width}px`,
								'--ob-comp-file-overlay-cover-inline-start': `${previewCover.left}px`,
							} satisfies CSSProperties
						}
					/>
				</>
			)}
		</>
	);
}
