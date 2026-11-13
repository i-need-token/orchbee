import type { ReactElement } from 'react';

import styles from './ProductPicture.module.css';

// Real captures from resources/readme/. The stills in public/pictures/ are re-encoded by
// the repository's scripts/generate-website-pictures.mjs; the animations are the README files themselves, copied to
// /pictures/ by the build, and visitors who ask for reduced motion get their last frame instead.
const PICTURES = {
	hero: {
		kind: 'still',
		width: 1600,
		height: 882,
		small: '/pictures/hero-800.webp',
		large: '/pictures/hero-1600.webp',
	},
	desktop: {
		kind: 'still',
		width: 1600,
		height: 888,
		small: '/pictures/desktop-800.webp',
		large: '/pictures/desktop-1600.webp',
	},
	multiplex: {
		kind: 'animation',
		width: 1100,
		height: 324,
		animation: '/pictures/multiplex.gif',
		still: '/pictures/multiplex-still.webp',
	},
	agents: {
		kind: 'animation',
		width: 1100,
		height: 605,
		animation: '/pictures/agents-cli.gif',
		still: '/pictures/agents-cli-still.webp',
	},
} as const;

// The widest frame is the 80rem page container less its 2.5rem gutters.
const SIZES = '(max-width: 80rem) calc(100vw - 2rem), 1200px';

export function ProductPicture({
	picture,
	alt,
	caption,
	priority = false,
}: {
	picture: keyof typeof PICTURES;
	alt: string;
	caption?: string;
	priority?: boolean;
}): ReactElement {
	const entry = PICTURES[picture];
	const loading = priority ? 'eager' : 'lazy';
	return (
		<figure className={styles['figure']}>
			{/* The captures show the dark product, so their frame stays dark in both page themes. */}
			<div className={styles['frame']} data-theme='dark'>
				{entry.kind === 'still' ? (
					<img
						className={styles['image']}
						src={entry.large}
						srcSet={`${entry.small} ${entry.width / 2}w, ${entry.large} ${entry.width}w`}
						sizes={SIZES}
						width={entry.width}
						height={entry.height}
						alt={alt}
						loading={loading}
						decoding='async'
						fetchPriority={priority ? 'high' : 'auto'}
					/>
				) : (
					<picture>
						<source media='(prefers-reduced-motion: reduce)' srcSet={entry.still} />
						<img
							className={styles['image']}
							src={entry.animation}
							width={entry.width}
							height={entry.height}
							alt={alt}
							loading={loading}
							decoding='async'
						/>
					</picture>
				)}
			</div>
			{caption === undefined ? null : (
				<figcaption className={styles['caption']}>{caption}</figcaption>
			)}
		</figure>
	);
}
