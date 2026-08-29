import type { TerminalImagePreviewPresentation } from '@orchbee/terminal';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalImagePreviewCard.module.css';
import { useTerminalImagePreviewCard } from './hooks/useTerminalImagePreviewCard';
import { terminalImageFileName } from './utils/terminalImageFileName';

type VisibleTerminalImagePreview = Extract<
	TerminalImagePreviewPresentation,
	{ readonly kind: 'visible' }
>;

export function ProjectedTerminalImagePreviewCard({
	preview,
}: {
	readonly preview: VisibleTerminalImagePreview;
}): ReactNode {
	const image = useTerminalImagePreviewCard(preview.content, preview.mediaType);
	const placement = preview.anchor.top > preview.anchor.surfaceHeight / 2 ? 'above' : 'below';
	if (image.failed) return null;

	return (
		<figure
			aria-hidden='true'
			className={styles['card']}
			data-placement={placement}
			data-slot='projected-terminal-image-preview-card'
			style={{
				'--ob-comp-projected-terminal-image-preview-anchor-block-size': `${preview.anchor.height}px`,
				'--ob-comp-projected-terminal-image-preview-block-start': `${preview.anchor.top}px`,
				'--ob-comp-projected-terminal-image-preview-inline-start': `${preview.anchor.left}px`,
			}}
		>
			<img
				alt=''
				className={styles['image']}
				draggable={false}
				onError={image.reportFailure}
				src={image.source}
			/>
			<figcaption className={styles['caption']}>{terminalImageFileName(preview.path)}</figcaption>
		</figure>
	);
}
