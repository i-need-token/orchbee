import type * as React from 'react';

import {
	terminalFontListWithFallback,
	type WindowTerminalColors,
} from '../../../terminal-settings';
import styles from './TerminalPreview.module.css';

interface TerminalPreviewProps {
	readonly colors: WindowTerminalColors;
	/** The CSS font list the terminal would draw with. */
	readonly fontFamily: string;
	readonly fontSize: number;
	readonly fontWeight: number;
	/** What assistive technology announces for the picture. */
	readonly label: string;
}

/** An ANSI color index, or the default text color. */
type Ink = number | 'foreground';

type PreviewLine = readonly (readonly [text: string, ink: Ink])[];

const BLOCK = '██';

/** A prompt, a listing in the usual ANSI colors, and the sixteen colors as cells. */
const PREVIEW_LINES: readonly PreviewLine[] = [
	[
		['~/projects/orchbee', 4],
		[' main', 5],
	],
	[
		['$ ', 2],
		['ls', 'foreground'],
	],
	[
		['src/', 4],
		['  build.sh', 2],
		['  README.md', 'foreground'],
		['  error.log', 1],
	],
	Array.from({ length: 8 }, (_, index) => [BLOCK, index] as const),
	Array.from({ length: 8 }, (_, index) => [BLOCK, index + 8] as const),
];

/** Line height as a multiple of the font size, close to a terminal cell's. */
const LINE_HEIGHT = 1.35;
/** The inset around the text, in CSS pixels; the SVG draws in CSS pixels. */
const INSET = 12;

/**
 * A few terminal lines in the candidate font, size and colors. The colors and the font are the
 * person's data, so they are drawn as SVG presentation attributes, as the theme swatches are.
 */
export function TerminalPreview({
	colors,
	fontFamily,
	fontSize,
	fontWeight,
	label,
}: TerminalPreviewProps): React.ReactNode {
	const lineHeight = Math.round(fontSize * LINE_HEIGHT);
	const height = INSET * 2 + PREVIEW_LINES.length * lineHeight;
	const ink = (value: Ink): string =>
		hexColor(
			value === 'foreground' ? colors.foreground : (colors.palette[value] ?? colors.foreground),
		);
	return (
		<svg aria-label={label} className={styles['preview']} height={height} role='img' width='100%'>
			<rect fill={hexColor(colors.background)} height='100%' width='100%' />
			<text
				dominantBaseline='central'
				fill={ink('foreground')}
				fontFamily={terminalFontListWithFallback(fontFamily)}
				fontSize={fontSize}
				fontWeight={fontWeight}
				xmlSpace='preserve'
			>
				{PREVIEW_LINES.map((line, row) => (
					<tspan key={row} x={INSET} y={INSET + row * lineHeight + lineHeight / 2}>
						{line.map(([text, value], column) => (
							<tspan fill={ink(value)} key={column}>
								{text}
							</tspan>
						))}
					</tspan>
				))}
			</text>
		</svg>
	);
}

function hexColor(rgb: number): string {
	return `#${rgb.toString(16).padStart(6, '0')}`;
}
