import type * as React from 'react';

import { cn } from '#lib';

import type { FileIconHue } from './file-icon-table.generated';
import styles from './FileIcon.module.css';
import { resolveFileIcon } from './resolveFileIcon';

// Each Seti hue's class in the stylesheet, which draws it through its file-type color token.
const hueClassKeys: Readonly<Record<FileIconHue, string>> = {
	blue: 'blue',
	green: 'green',
	grey: 'grey',
	'grey-light': 'greyLight',
	ignore: 'ignore',
	orange: 'orange',
	pink: 'pink',
	purple: 'purple',
	red: 'red',
	white: 'white',
	yellow: 'yellow',
};

interface FileIconProps {
	/** The file's name. */
	readonly name: string;
	/** The file's path; a few language associations match folders in it. Defaults to the name. */
	readonly path?: string;
}

/** The decorative Seti icon VS Code draws for a file; the owning row supplies the accessible name. */
export function FileIcon({ name, path }: FileIconProps): React.ReactNode {
	const [glyph, hue] = resolveFileIcon(name, path);
	return (
		<span
			aria-hidden='true'
			className={cn(styles['root'], hue !== null && styles[hueClassKeys[hue]])}
			data-file-icon={glyph}
		/>
	);
}
