import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { Icon } from '../icon/Icon';
import styles from './ProductRecording.module.css';

// A screen recording from resources/readme, re-encoded as a muted WebM by
// scripts/generate-website-pictures.mjs, with its last frame as the poster. Nothing loads until
// the page script plays it in view; under reduced motion it stays on the poster until the visitor
// presses play.
export function ProductRecording({
	src,
	poster,
	width,
	height,
	label,
}: {
	src: string;
	poster: string;
	width: number;
	height: number;
	label: string;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['recording']} data-recording=''>
			<video
				className={styles['video']}
				poster={poster}
				width={width}
				height={height}
				preload='none'
				aria-label={label}
				muted
				loop
				playsInline
				disablePictureInPicture
			>
				<source src={src} type='video/webm' />
			</video>
			<button
				className={styles['toggle']}
				type='button'
				data-recording-toggle=''
				data-label-pause={t(messages.pauseRecording)}
				data-label-play={t(messages.playRecording)}
				aria-label={t(messages.playRecording)}
				title={t(messages.playRecording)}
				hidden
			>
				<span className={styles['pause']}>
					<Icon name='pause' size={12} />
				</span>
				<span className={styles['play']}>
					<Icon name='play' size={12} />
				</span>
			</button>
		</div>
	);
}
