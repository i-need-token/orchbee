import type { ReactElement } from 'react';

import styles from './ProductPicture.module.css';

export interface PictureFile {
	readonly src: string;
	readonly width: number;
	readonly height: number;
}

// A screenshot from resources/readme, derived by scripts/generate-website-pictures.mjs. The
// browser picks a size from `files`; on narrow screens `narrow` swaps in a crop that stays
// readable. Pictures below the first screen load lazily.
export function ProductPicture({
	files,
	narrow,
	sizes,
	alt,
	eager = false,
}: {
	files: readonly [PictureFile, ...PictureFile[]];
	narrow?: PictureFile;
	sizes: string;
	alt: string;
	eager?: boolean;
}): ReactElement {
	const [largest] = files;
	return (
		<picture className={styles['picture']}>
			{narrow === undefined ? null : (
				<source
					media='(max-width: 40rem)'
					srcSet={narrow.src}
					width={narrow.width}
					height={narrow.height}
				/>
			)}
			<img
				className={styles['image']}
				src={largest.src}
				srcSet={files.map((file) => `${file.src} ${String(file.width)}w`).join(', ')}
				sizes={sizes}
				width={largest.width}
				height={largest.height}
				alt={alt}
				loading={eager ? 'eager' : 'lazy'}
				fetchPriority={eager ? 'high' : 'auto'}
				decoding='async'
			/>
		</picture>
	);
}
