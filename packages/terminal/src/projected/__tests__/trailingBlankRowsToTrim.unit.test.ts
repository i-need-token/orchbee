import {
	ActiveScreen,
	CursorShape,
	TerminalColorEncoding,
	TerminalUnderlineKindCode,
	type TerminalGraphicsPlacement,
} from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import type { ProjectedTerminalMetadata } from '../packed-terminal-store';
import { trailingBlankRowsToTrim } from '../trailingBlankRowsToTrim';

const interaction: ProjectedTerminalMetadata['interaction'] = {
	alternateScroll: false,
	applicationCursorKeys: false,
	applicationKeypad: false,
	bracketedPaste: false,
	focusReporting: false,
	kittyDisambiguateEscapeCodes: false,
	kittyReportAllKeysAsEscapeCodes: false,
	kittyReportAlternateKeys: false,
	kittyReportAssociatedText: false,
	kittyReportEventTypes: false,
	modifyOtherKeys2: false,
	mouseAny: false,
	mouseButton: false,
	mouseNormal: false,
	mouseSgr: false,
	mouseSgrPixels: false,
	mouseUtf8: false,
	mouseUrxvt: false,
	mouseX10: false,
};

function metadata(
	rows: number,
	overrides: Partial<ProjectedTerminalMetadata> = {},
): ProjectedTerminalMetadata {
	return {
		activeScreen: ActiveScreen.primary,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 2,
		commandCompletion: null,
		closeActivity: 'active',
		currentDirectory: null,
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
			shape: CursorShape.block,
			visible: false,
			wideTail: false,
		},
		defaultColors: { backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
		generation: 1n,
		interaction,
		progress: { state: 'remove', value: null },
		revision: 1n,
		rows,
		scrollbackRows: 20n,
		themeRevision: 1n,
		title: null,
		...overrides,
	};
}

function row(
	text = '',
	overrides: Partial<
		Omit<Parameters<typeof trailingBlankRowsToTrim>[0]['rows'][number], 'grapheme'>
	> = {},
): Parameters<typeof trailingBlankRowsToTrim>[0]['rows'][number] {
	return {
		backgroundColors: new Uint32Array(2).fill(TerminalColorEncoding.default),
		grapheme: (column) => (column === 0 ? text : ''),
		promptClass: 0,
		rowFlags: 0,
		styleFlags: new Uint16Array(2),
		underlineColors: new Uint32Array(2).fill(TerminalColorEncoding.none),
		underlineKinds: new Uint8Array(2).fill(TerminalUnderlineKindCode.none),
		...overrides,
	};
}

function placement(rowIndex: number): TerminalGraphicsPlacement {
	return {
		column: 0,
		destinationHeight: 18,
		destinationWidth: 9,
		imageId: 1,
		internalId: false,
		offsetX: 0,
		offsetY: 0,
		placementId: 1,
		row: rowIndex,
		sourceHeight: 1,
		sourceWidth: 1,
		sourceX: 0,
		sourceY: 0,
		z: 0,
	};
}

describe('trailingBlankRowsToTrim', () => {
	it('trims only the contiguous blank suffix and keeps one live row', () => {
		const rows = [row('output'), row(''), row(' '), row('')];
		expect(
			trailingBlankRowsToTrim({ enabled: true, metadata: metadata(4), placements: [], rows }),
		).toBe(3);
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(4, { scrollbackRows: 2n }),
				placements: [],
				rows,
			}),
		).toBe(2);
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(3),
				placements: [],
				rows: [row(), row(), row()],
			}),
		).toBe(2);
	});

	it('preserves internal blank rows and every visible tail signal', () => {
		const styledBackground = new Uint32Array(2).fill(TerminalColorEncoding.default);
		styledBackground[0] = 0x11_22_33;
		const styleFlags = new Uint16Array(2);
		styleFlags[0] = 1;
		const underlineKinds = new Uint8Array(2);
		underlineKinds[0] = TerminalUnderlineKindCode.single;
		const underlineColors = new Uint32Array(2).fill(TerminalColorEncoding.none);
		underlineColors[0] = TerminalColorEncoding.default;
		const visibleTailRows = [
			row('x'),
			row('', { backgroundColors: styledBackground }),
			row('', { styleFlags }),
			row('', { underlineKinds }),
			row('', { underlineColors }),
			row('', { promptClass: 1 }),
			row('', { rowFlags: 1 }),
		];
		for (const tail of visibleTailRows) {
			expect(
				trailingBlankRowsToTrim({
					enabled: true,
					metadata: metadata(3),
					placements: [],
					rows: [row('top'), row(), tail],
				}),
			).toBe(0);
		}
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(4),
				placements: [],
				rows: [row('top'), row(), row('middle'), row()],
			}),
		).toBe(1);
	});

	it('treats the cursor and all rows intersected by graphics as visible', () => {
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(3, { cursor: { ...metadata(3).cursor, row: 2, visible: true } }),
				placements: [],
				rows: [row('top'), row(), row()],
			}),
		).toBe(0);
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(4),
				placements: [{ ...placement(2), destinationHeight: 36 }],
				rows: [row('top'), row(), row(), row()],
			}),
		).toBe(0);
	});

	it('is a no-op outside the safe primary-screen policy boundary', () => {
		const rows = [row('top'), row(), row()];
		expect(
			trailingBlankRowsToTrim({ enabled: false, metadata: metadata(3), placements: [], rows }),
		).toBe(0);
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(3, { activeScreen: ActiveScreen.alternate }),
				placements: [],
				rows,
			}),
		).toBe(0);
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(3, { interaction: { ...interaction, mouseNormal: true } }),
				placements: [],
				rows,
			}),
		).toBe(0);
		expect(
			trailingBlankRowsToTrim({
				enabled: true,
				metadata: metadata(3, { scrollbackRows: 0n }),
				placements: [],
				rows,
			}),
		).toBe(0);
	});
});
