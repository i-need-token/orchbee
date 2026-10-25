import { z } from 'zod';

import { BinaryReader, BinaryWriter, checkedAdd, checkedMultiply } from './binary';
import {
	AttachmentIdSchema,
	HostEpochSchema,
	IDENTIFIER_BYTE_LENGTH,
	identifierBytes,
	identifierText,
	parseAttachmentId,
	parseHostEpoch,
} from './identifiers';
import type { AttachmentId, HostEpoch } from './identifiers';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';
import {
	decodeTerminalGraphics,
	TerminalGraphicsUpdateKind,
	type TerminalGraphics,
} from './terminal-graphics';
import { containsUnpairedSurrogate } from './containsUnpairedSurrogate';

const TERMINAL_FRAME_HEADER_BYTES = 160;
const TERMINAL_FRAME_MAGIC = new Uint8Array([0x4f, 0x42, 0x54, 0x46]);
// Keep this synchronized with schemas/terminal/v1/constants.json, Rust and Zig. Released changes (>= 1.0.0) also update
// schemas/terminal/docs/version-history.md in the same change.
export const TERMINAL_FRAME_FORMAT = { major: 0, minor: 1, patch: 0 } as const;

const MAX_UINT16 = 0xffff;
const MAX_UINT32 = 0xffff_ffff;
const MAX_UINT64 = 0xffff_ffff_ffff_ffffn;
const STYLE_RECORD_BYTES = 20;
const ROW_HEADER_BYTES = 8;
const CELL_RECORD_BYTES = 8;
const EXTENSION_HEADER_BYTES = 8;
const COLOR_RGB_MASK = 0x00ff_ffff;
const COLOR_TAG_MASK = 0xff00_0000;
const COLOR_DEFAULT = 0x0100_0000;
const COLOR_NONE = 0x0200_0000;
const CURSOR_FLAGS_MASK = 0b111;
const ROW_FLAGS_MASK = 0b11;
const CELL_FLAGS_MASK = 0b1_1111;
const STYLE_FLAGS_MASK = 0x00ff;
const INTERACTION_FLAGS_MASK = 0x0007_ffff;

export const TerminalFrameKind = {
	delta: 'delta',
	snapshot: 'snapshot',
} as const;

export const CursorShape = {
	bar: 'bar',
	block: 'block',
	underline: 'underline',
} as const;

export const ActiveScreen = {
	alternate: 'alternate',
	primary: 'primary',
} as const;

export const CellWidth = {
	narrow: 'narrow',
	spacerHead: 'spacer-head',
	spacerTail: 'spacer-tail',
	wide: 'wide',
} as const;

export const SemanticContent = {
	input: 'input',
	output: 'output',
	prompt: 'prompt',
} as const;

export const RowPromptClass = {
	none: 'none',
	prompt: 'prompt',
	promptContinuation: 'prompt-continuation',
} as const;

export const UnderlineKind = {
	curly: 'curly',
	dashed: 'dashed',
	dotted: 'dotted',
	double: 'double',
	none: 'none',
	single: 'single',
} as const;

export const TerminalColorKind = {
	default: 'default',
	none: 'none',
	rgb: 'rgb',
} as const;

export const TerminalColorEncoding = {
	default: COLOR_DEFAULT,
	none: COLOR_NONE,
	rgbMask: COLOR_RGB_MASK,
} as const;

export const TerminalCellWidthCode = {
	narrow: 0,
	spacerHead: 3,
	spacerTail: 2,
	wide: 1,
} as const;

export const TerminalSemanticContentCode = {
	input: 1,
	output: 0,
	prompt: 2,
} as const;

export const TerminalRowPromptClassCode = {
	none: 0,
	prompt: 1,
	promptContinuation: 2,
} as const;

export const TerminalRowFlag = {
	wrap: 1,
	wrapContinuation: 1 << 1,
} as const;

export const TerminalFrameExtensionKind = {
	commandCompletion: 4,
	closeActivity: 1,
	kittyGraphics: 2,
	progress: 3,
} as const;

export const TerminalCloseActivityCode = {
	active: 2,
	idle: 1,
	unknown: 0,
} as const;

const TerminalProgressState = {
	error: 'error',
	indeterminate: 'indeterminate',
	pause: 'pause',
	remove: 'remove',
	set: 'set',
} as const;

type TerminalProgressState = (typeof TerminalProgressState)[keyof typeof TerminalProgressState];

export interface TerminalProgress {
	readonly state: TerminalProgressState;
	readonly value: number | null;
}

export interface TerminalCommandCompletion {
	readonly completedAtMilliseconds: bigint;
	readonly durationMilliseconds: bigint;
	readonly exitCode: number;
	readonly sequence: bigint;
}

const TerminalProgressStateCode = {
	error: 2,
	indeterminate: 3,
	pause: 4,
	remove: 0,
	set: 1,
} as const;

export const TerminalStyleFlag = {
	blink: 1 << 3,
	bold: 1,
	faint: 1 << 2,
	inverse: 1 << 4,
	invisible: 1 << 5,
	italic: 1 << 1,
	overline: 1 << 7,
	strikethrough: 1 << 6,
} as const;

export const TerminalUnderlineKindCode = {
	curly: 3,
	dashed: 5,
	dotted: 4,
	double: 2,
	none: 0,
	single: 1,
} as const;

export type ActiveScreen = (typeof ActiveScreen)[keyof typeof ActiveScreen];
export type CellWidth = (typeof CellWidth)[keyof typeof CellWidth];
export type CursorShape = (typeof CursorShape)[keyof typeof CursorShape];
export type RowPromptClass = (typeof RowPromptClass)[keyof typeof RowPromptClass];
export type SemanticContent = (typeof SemanticContent)[keyof typeof SemanticContent];
export type TerminalFrameKind = (typeof TerminalFrameKind)[keyof typeof TerminalFrameKind];
export type UnderlineKind = (typeof UnderlineKind)[keyof typeof UnderlineKind];

const uint16Schema = z.number().int().min(0).max(MAX_UINT16);
const rgbSchema = z.number().int().min(0).max(COLOR_RGB_MASK);
const uint64Schema = z.bigint().min(0n).max(MAX_UINT64);
const positiveUint64Schema = uint64Schema.refine((value) => value > 0n, {
	message: 'Value must be greater than zero',
});

const TerminalColorSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal(TerminalColorKind.default) }),
	z.strictObject({ kind: z.literal(TerminalColorKind.none) }),
	z.strictObject({ kind: z.literal(TerminalColorKind.rgb), rgb: rgbSchema }),
]);

const TerminalInteractionSchema = z.strictObject({
	alternateScroll: z.boolean(),
	applicationCursorKeys: z.boolean(),
	applicationKeypad: z.boolean(),
	bracketedPaste: z.boolean(),
	focusReporting: z.boolean(),
	kittyDisambiguateEscapeCodes: z.boolean(),
	kittyReportAllKeysAsEscapeCodes: z.boolean(),
	kittyReportAlternateKeys: z.boolean(),
	kittyReportAssociatedText: z.boolean(),
	kittyReportEventTypes: z.boolean(),
	modifyOtherKeys2: z.boolean(),
	mouseAny: z.boolean(),
	mouseButton: z.boolean(),
	mouseNormal: z.boolean(),
	mouseSgr: z.boolean(),
	mouseSgrPixels: z.boolean(),
	mouseUtf8: z.boolean(),
	mouseUrxvt: z.boolean(),
	mouseX10: z.boolean(),
});

const TerminalCellSchema = z.strictObject({
	grapheme: z.string(),
	protected: z.boolean(),
	semanticContent: z.enum(SemanticContent),
	style: uint16Schema,
	width: z.enum(CellWidth),
});

const TerminalStyleSchema = z.strictObject({
	background: TerminalColorSchema,
	blink: z.boolean(),
	bold: z.boolean(),
	faint: z.boolean(),
	foreground: TerminalColorSchema,
	hyperlink: z.string().nullable(),
	inverse: z.boolean(),
	invisible: z.boolean(),
	italic: z.boolean(),
	overline: z.boolean(),
	strikethrough: z.boolean(),
	underline: TerminalColorSchema,
	underlineKind: z.enum(UnderlineKind),
});

const TerminalRowSchema = z.strictObject({
	cells: z.array(TerminalCellSchema).max(PROTOCOL_LIMITS.maxFrameColumns),
	index: uint16Schema,
	promptClass: z.enum(RowPromptClass),
	wrap: z.boolean(),
	wrapContinuation: z.boolean(),
});

const TerminalFrameExtensionSchema = z.strictObject({
	data: z.instanceof(Uint8Array),
	kind: uint16Schema,
	required: z.boolean(),
});

export const TerminalFrameSchema = z
	.strictObject({
		activeScreen: z.enum(ActiveScreen),
		attachmentId: AttachmentIdSchema,
		baseRevision: uint64Schema.nullable(),
		cellPixelHeight: z.number().int().min(0).max(PROTOCOL_LIMITS.maxCellPixelDimension),
		cellPixelWidth: z.number().int().min(0).max(PROTOCOL_LIMITS.maxCellPixelDimension),
		columns: z.number().int().min(1).max(PROTOCOL_LIMITS.maxFrameColumns),
		cursor: z.strictObject({
			blinking: z.boolean(),
			column: uint16Schema,
			row: uint16Schema,
			shape: z.enum(CursorShape),
			visible: z.boolean(),
			wideTail: z.boolean(),
		}),
		currentDirectory: z.string().nullable(),
		defaultColors: z.strictObject({
			backgroundRgb: rgbSchema,
			cursorRgb: rgbSchema.nullable(),
			foregroundRgb: rgbSchema,
		}),
		extensions: z.array(TerminalFrameExtensionSchema).max(PROTOCOL_LIMITS.maxFrameExtensions),
		generation: positiveUint64Schema,
		hostEpoch: HostEpochSchema,
		interaction: TerminalInteractionSchema,
		kind: z.enum(TerminalFrameKind),
		revision: uint64Schema,
		rowData: z.array(TerminalRowSchema).max(PROTOCOL_LIMITS.maxFrameRows),
		rows: z.number().int().min(1).max(PROTOCOL_LIMITS.maxFrameRows),
		scrollbackRows: uint64Schema,
		sequence: positiveUint64Schema,
		styles: z.array(TerminalStyleSchema).max(PROTOCOL_LIMITS.maxFrameStyles),
		themeRevision: uint64Schema,
		title: z.string().nullable(),
	})
	.superRefine((frame, context) => {
		const stringValidationCache = new Map<string, StringByteValidation>();
		if (frame.kind === TerminalFrameKind.snapshot && frame.baseRevision !== null) {
			addIssue(context, ['baseRevision'], 'Snapshot frames must not declare a base revision');
		}
		if (
			frame.kind === TerminalFrameKind.delta &&
			(frame.baseRevision === null || frame.baseRevision >= frame.revision)
		) {
			addIssue(
				context,
				['baseRevision'],
				'Delta frames require a base revision lower than their revision',
			);
		}
		if (frame.cursor.row >= frame.rows || frame.cursor.column >= frame.columns) {
			addIssue(context, ['cursor'], 'Cursor position must be inside the terminal grid');
		}
		if (frame.columns * frame.rows > PROTOCOL_LIMITS.maxFrameCells) {
			addIssue(context, ['columns'], `Grid exceeds ${PROTOCOL_LIMITS.maxFrameCells} cells`);
		}
		if ((frame.cellPixelWidth === 0) !== (frame.cellPixelHeight === 0)) {
			addIssue(context, ['cellPixelWidth'], 'Cell pixels must be both known or both unknown');
		}
		if (frame.columns * frame.cellPixelWidth > MAX_UINT16) {
			addIssue(context, ['cellPixelWidth'], 'Terminal pixel width exceeds 65535');
		}
		if (frame.rows * frame.cellPixelHeight > MAX_UINT16) {
			addIssue(context, ['cellPixelHeight'], 'Terminal pixel height exceeds 65535');
		}
		if (frame.kind === TerminalFrameKind.snapshot && frame.rowData.length !== frame.rows) {
			addIssue(context, ['rowData'], 'Snapshot frames must contain every visible row');
		}
		const encodedInteraction = interactionFlags(frame.interaction);
		const mouseModes = [4, 5, 6, 7].filter((bit) => (encodedInteraction & (1 << bit)) !== 0).length;
		const mouseFormats = [8, 9, 10, 11].filter(
			(bit) => (encodedInteraction & (1 << bit)) !== 0,
		).length;
		if (mouseModes > 1 || mouseFormats > 1) {
			addIssue(context, ['interaction'], 'Mouse reporting modes and formats must be exclusive');
		}

		validateStringBytes(frame.title, ['title'], context, stringValidationCache);
		validateStringBytes(
			frame.currentDirectory,
			['currentDirectory'],
			context,
			stringValidationCache,
		);
		for (const [index, style] of frame.styles.entries()) {
			validateStringBytes(
				style.hyperlink,
				['styles', index, 'hyperlink'],
				context,
				stringValidationCache,
			);
		}

		let previousRow = -1;
		let totalCells = 0;
		for (const [rowOffset, row] of frame.rowData.entries()) {
			totalCells += row.cells.length;
			if (row.index >= frame.rows || row.index <= previousRow) {
				addIssue(context, ['rowData', rowOffset, 'index'], 'Rows must be unique and sorted');
			}
			if (frame.kind === TerminalFrameKind.snapshot && row.index !== rowOffset) {
				addIssue(context, ['rowData', rowOffset, 'index'], 'Snapshot rows must be contiguous');
			}
			if (row.cells.length !== frame.columns) {
				addIssue(
					context,
					['rowData', rowOffset, 'cells'],
					'Every row must contain exactly columns cells',
				);
			}
			previousRow = row.index;
			const priorRow = frame.rowData[rowOffset - 1];
			if (
				priorRow !== undefined &&
				row.index === priorRow.index + 1 &&
				priorRow.wrap !== row.wrapContinuation
			) {
				addIssue(
					context,
					['rowData', rowOffset, 'wrapContinuation'],
					'Adjacent row wrap flags must agree',
				);
			}
			if (
				priorRow?.cells.at(-1)?.width === CellWidth.spacerHead &&
				row.index === priorRow.index + 1 &&
				row.cells[0]?.width !== CellWidth.wide
			) {
				addIssue(
					context,
					['rowData', rowOffset, 'cells', 0, 'width'],
					'A row after spacer-head must begin with the wrapped wide cell',
				);
			}
			for (const [cellOffset, cell] of row.cells.entries()) {
				validateStringBytes(
					cell.grapheme,
					['rowData', rowOffset, 'cells', cellOffset, 'grapheme'],
					context,
					stringValidationCache,
				);
				if (cell.style >= frame.styles.length) {
					addIssue(
						context,
						['rowData', rowOffset, 'cells', cellOffset, 'style'],
						'Cell style index is outside the style table',
					);
				}
				validateWidthSequence(row, rowOffset, cellOffset, context);
			}
		}
		if (totalCells > PROTOCOL_LIMITS.maxFrameCells) {
			addIssue(context, ['rowData'], `Frame exceeds ${PROTOCOL_LIMITS.maxFrameCells} cells`);
		}

		let extensionBytes = 0;
		for (const [index, extension] of frame.extensions.entries()) {
			if (extension.required) {
				addIssue(
					context,
					['extensions', index, 'required'],
					'This frame format has no recognized required extensions',
				);
			}
			extensionBytes += extension.data.byteLength;
			if (
				extension.data.byteLength > PROTOCOL_LIMITS.maxExtensionBytes ||
				extensionBytes > PROTOCOL_LIMITS.maxExtensionBytes
			) {
				addIssue(context, ['extensions', index, 'data'], 'Frame extensions exceed their limit');
			}
		}
		const graphics = graphicsFromExtensions(frame.extensions, frame.kind);
		if (!graphics.ok) addIssue(context, ['extensions'], graphics.detail);
		const progress = progressFromExtensions(frame.extensions);
		if (!progress.ok) addIssue(context, ['extensions'], progress.detail);
		const commandCompletion = commandCompletionFromExtensions(frame.extensions);
		if (!commandCompletion.ok) addIssue(context, ['extensions'], commandCompletion.detail);
	});

export type TerminalColor = z.infer<typeof TerminalColorSchema>;
export type TerminalFrame = z.infer<typeof TerminalFrameSchema>;
type TerminalFrameExtension = z.infer<typeof TerminalFrameExtensionSchema>;
export type TerminalInteraction = z.infer<typeof TerminalInteractionSchema>;
type TerminalRow = z.infer<typeof TerminalRowSchema>;
export type TerminalStyle = z.infer<typeof TerminalStyleSchema>;

interface DecodedTerminalStyles {
	readonly backgroundColors: Uint32Array;
	readonly flags: Uint16Array;
	readonly foregroundColors: Uint32Array;
	readonly hyperlinkStringIndices: Uint32Array;
	readonly underlineColors: Uint32Array;
	readonly underlineKinds: Uint8Array;
}

interface DecodedTerminalRows {
	readonly cellOffsets: Uint32Array;
	readonly flags: Uint8Array;
	readonly indices: Uint16Array;
	readonly promptClasses: Uint8Array;
}

interface DecodedTerminalCells {
	readonly graphemeStringIndices: Uint32Array;
	readonly protected: Uint8Array;
	readonly semanticContents: Uint8Array;
	readonly styleIndices: Uint16Array;
	readonly widths: Uint8Array;
}

export interface DecodedTerminalFrame {
	readonly activeScreen: ActiveScreen;
	readonly attachmentId: AttachmentId;
	readonly baseRevision: bigint | null;
	readonly cellPixelHeight: number;
	readonly cellPixelWidth: number;
	readonly cells: DecodedTerminalCells;
	readonly columns: number;
	readonly commandCompletion: TerminalCommandCompletion | null;
	readonly currentDirectory: string | null;
	readonly cursor: {
		readonly blinking: boolean;
		readonly column: number;
		readonly row: number;
		readonly shape: CursorShape;
		readonly visible: boolean;
		readonly wideTail: boolean;
	};
	readonly defaultColors: {
		readonly backgroundRgb: number;
		readonly cursorRgb: number | null;
		readonly foregroundRgb: number;
	};
	readonly extensions: readonly TerminalFrameExtension[];
	readonly generation: bigint;
	readonly graphics: TerminalGraphics | null;
	readonly hostEpoch: HostEpoch;
	readonly ignoredOptionalFlags: number;
	readonly interaction: TerminalInteraction;
	readonly kind: TerminalFrameKind;
	readonly progress: TerminalProgress;
	readonly revision: bigint;
	readonly rowData: DecodedTerminalRows;
	readonly rows: number;
	readonly scrollbackRows: bigint;
	readonly sequence: bigint;
	readonly strings: readonly string[];
	readonly styles: DecodedTerminalStyles;
	readonly themeRevision: bigint;
	readonly title: string | null;
}

interface DecodedTerminalFrameRoute {
	readonly attachmentId: AttachmentId;
	readonly hostEpoch: HostEpoch;
}

export interface InspectedTerminalFrameHeader {
	readonly attachmentId: AttachmentId;
	readonly baseRevision: bigint | null;
	readonly generation: bigint;
	readonly hostEpoch: HostEpoch;
	readonly kind: TerminalFrameKind;
	readonly revision: bigint;
	readonly sequence: bigint;
	readonly themeRevision: bigint;
}

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

interface StringByteValidation {
	readonly issue: string | null;
}

function addIssue(context: z.RefinementCtx, path: readonly PropertyKey[], message: string): void {
	context.addIssue({ code: 'custom', message, path: [...path] });
}

function validateStringBytes(
	value: string | null,
	path: readonly PropertyKey[],
	context: z.RefinementCtx,
	cache: Map<string, StringByteValidation>,
): void {
	if (value === null) return;
	let validation = cache.get(value);
	if (validation === undefined) {
		let issue: string | null = null;
		if (containsUnpairedSurrogate(value)) {
			issue = 'String contains an unpaired UTF-16 surrogate';
		} else if (textEncoder.encode(value).byteLength > PROTOCOL_LIMITS.maxStringBytes) {
			issue = `String exceeds ${PROTOCOL_LIMITS.maxStringBytes} UTF-8 bytes`;
		}
		validation = { issue };
		cache.set(value, validation);
	}
	if (validation.issue !== null) addIssue(context, path, validation.issue);
}

function validateWidthSequence(
	row: TerminalRow,
	rowOffset: number,
	cellOffset: number,
	context: z.RefinementCtx,
): void {
	const cells = row.cells;
	const cell = cells[cellOffset];
	if (!cell) return;
	const previous = cells[cellOffset - 1];
	const next = cells[cellOffset + 1];
	if (cell.width === CellWidth.wide && next?.width !== CellWidth.spacerTail) {
		addIssue(
			context,
			['rowData', rowOffset, 'cells', cellOffset, 'width'],
			'Wide cells must be followed by spacer-tail',
		);
	}
	if (cell.width === CellWidth.spacerTail && previous?.width !== CellWidth.wide) {
		addIssue(
			context,
			['rowData', rowOffset, 'cells', cellOffset, 'width'],
			'Spacer-tail cells must follow a wide cell',
		);
	}
	if (cell.width === CellWidth.spacerHead && (cellOffset !== cells.length - 1 || !row.wrap)) {
		addIssue(
			context,
			['rowData', rowOffset, 'cells', cellOffset, 'width'],
			'Spacer-head is valid only in the last column of a wrapped row',
		);
	}
}

function frameKindCode(kind: TerminalFrameKind): number {
	return kind === TerminalFrameKind.snapshot ? 1 : 2;
}

function frameKindFromCode(code: number): ProtocolResult<TerminalFrameKind> {
	switch (code) {
		case 1:
			return protocolSuccess(TerminalFrameKind.snapshot);
		case 2:
			return protocolSuccess(TerminalFrameKind.delta);
		default:
			return protocolFailure(ProtocolErrorCode.invalidField, `Unknown terminal frame kind ${code}`);
	}
}

function cursorShapeCode(shape: CursorShape): number {
	switch (shape) {
		case CursorShape.block:
			return 1;
		case CursorShape.bar:
			return 2;
		case CursorShape.underline:
			return 3;
	}
}

function cursorShapeFromCode(code: number): ProtocolResult<CursorShape> {
	switch (code) {
		case 1:
			return protocolSuccess(CursorShape.block);
		case 2:
			return protocolSuccess(CursorShape.bar);
		case 3:
			return protocolSuccess(CursorShape.underline);
		default:
			return protocolFailure(ProtocolErrorCode.invalidField, `Unknown cursor shape ${code}`);
	}
}

function screenCode(screen: ActiveScreen): number {
	return screen === ActiveScreen.primary ? 1 : 2;
}

function screenFromCode(code: number): ProtocolResult<ActiveScreen> {
	switch (code) {
		case 1:
			return protocolSuccess(ActiveScreen.primary);
		case 2:
			return protocolSuccess(ActiveScreen.alternate);
		default:
			return protocolFailure(ProtocolErrorCode.invalidField, `Unknown active screen ${code}`);
	}
}

function cellWidthCode(width: CellWidth): number {
	switch (width) {
		case CellWidth.narrow:
			return 0;
		case CellWidth.wide:
			return 1;
		case CellWidth.spacerTail:
			return 2;
		case CellWidth.spacerHead:
			return 3;
	}
}

function semanticContentCode(content: SemanticContent): number {
	switch (content) {
		case SemanticContent.output:
			return 0;
		case SemanticContent.input:
			return 1;
		case SemanticContent.prompt:
			return 2;
	}
}

function promptClassCode(promptClass: RowPromptClass): number {
	switch (promptClass) {
		case RowPromptClass.none:
			return 0;
		case RowPromptClass.prompt:
			return 1;
		case RowPromptClass.promptContinuation:
			return 2;
	}
}

function underlineKindCode(kind: UnderlineKind): number {
	switch (kind) {
		case UnderlineKind.none:
			return 0;
		case UnderlineKind.single:
			return 1;
		case UnderlineKind.double:
			return 2;
		case UnderlineKind.curly:
			return 3;
		case UnderlineKind.dotted:
			return 4;
		case UnderlineKind.dashed:
			return 5;
	}
}

function colorCode(color: TerminalColor): number {
	switch (color.kind) {
		case TerminalColorKind.default:
			return COLOR_DEFAULT;
		case TerminalColorKind.none:
			return COLOR_NONE;
		case TerminalColorKind.rgb:
			return color.rgb;
	}
}

function validColorCode(color: number): boolean {
	const tag = color & COLOR_TAG_MASK;
	return tag === 0 || color === COLOR_DEFAULT || color === COLOR_NONE;
}

function styleFlags(style: TerminalStyle): number {
	return (
		(style.bold ? 1 : 0) |
		(style.italic ? 1 << 1 : 0) |
		(style.faint ? 1 << 2 : 0) |
		(style.blink ? 1 << 3 : 0) |
		(style.inverse ? 1 << 4 : 0) |
		(style.invisible ? 1 << 5 : 0) |
		(style.strikethrough ? 1 << 6 : 0) |
		(style.overline ? 1 << 7 : 0)
	);
}

function interactionFlags(interaction: TerminalInteraction): number {
	return (
		(interaction.applicationCursorKeys ? 1 : 0) |
		(interaction.applicationKeypad ? 1 << 1 : 0) |
		(interaction.bracketedPaste ? 1 << 2 : 0) |
		(interaction.focusReporting ? 1 << 3 : 0) |
		(interaction.mouseX10 ? 1 << 4 : 0) |
		(interaction.mouseNormal ? 1 << 5 : 0) |
		(interaction.mouseButton ? 1 << 6 : 0) |
		(interaction.mouseAny ? 1 << 7 : 0) |
		(interaction.mouseUtf8 ? 1 << 8 : 0) |
		(interaction.mouseSgr ? 1 << 9 : 0) |
		(interaction.mouseUrxvt ? 1 << 10 : 0) |
		(interaction.mouseSgrPixels ? 1 << 11 : 0) |
		(interaction.modifyOtherKeys2 ? 1 << 12 : 0) |
		(interaction.kittyDisambiguateEscapeCodes ? 1 << 13 : 0) |
		(interaction.kittyReportEventTypes ? 1 << 14 : 0) |
		(interaction.kittyReportAlternateKeys ? 1 << 15 : 0) |
		(interaction.kittyReportAllKeysAsEscapeCodes ? 1 << 16 : 0) |
		(interaction.kittyReportAssociatedText ? 1 << 17 : 0) |
		(interaction.alternateScroll ? 1 << 18 : 0)
	);
}

function interactionFromFlags(flags: number): ProtocolResult<TerminalInteraction> {
	if ((flags & ~INTERACTION_FLAGS_MASK) !== 0) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Terminal interaction state has unknown required bits',
		);
	}
	const mouseModes = [4, 5, 6, 7].filter((bit) => (flags & (1 << bit)) !== 0).length;
	const mouseFormats = [8, 9, 10, 11].filter((bit) => (flags & (1 << bit)) !== 0).length;
	if (mouseModes > 1 || mouseFormats > 1) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal interaction state declares conflicting mouse modes',
		);
	}
	return protocolSuccess({
		alternateScroll: (flags & (1 << 18)) !== 0,
		applicationCursorKeys: (flags & 1) !== 0,
		applicationKeypad: (flags & (1 << 1)) !== 0,
		bracketedPaste: (flags & (1 << 2)) !== 0,
		focusReporting: (flags & (1 << 3)) !== 0,
		kittyDisambiguateEscapeCodes: (flags & (1 << 13)) !== 0,
		kittyReportAllKeysAsEscapeCodes: (flags & (1 << 16)) !== 0,
		kittyReportAlternateKeys: (flags & (1 << 15)) !== 0,
		kittyReportAssociatedText: (flags & (1 << 17)) !== 0,
		kittyReportEventTypes: (flags & (1 << 14)) !== 0,
		modifyOtherKeys2: (flags & (1 << 12)) !== 0,
		mouseAny: (flags & (1 << 7)) !== 0,
		mouseButton: (flags & (1 << 6)) !== 0,
		mouseNormal: (flags & (1 << 5)) !== 0,
		mouseSgr: (flags & (1 << 9)) !== 0,
		mouseSgrPixels: (flags & (1 << 11)) !== 0,
		mouseUtf8: (flags & (1 << 8)) !== 0,
		mouseUrxvt: (flags & (1 << 10)) !== 0,
		mouseX10: (flags & (1 << 4)) !== 0,
	});
}

function isMagic(bytes: Uint8Array): boolean {
	return TERMINAL_FRAME_MAGIC.every((value, index) => bytes[index] === value);
}

class StringTableBuilder {
	readonly #indices = new Map<string, number>();
	readonly #strings: string[] = [];

	add(value: string | null): number {
		if (value === null) return MAX_UINT32;
		const existing = this.#indices.get(value);
		if (existing !== undefined) return existing;
		const index = this.#strings.length;
		this.#strings.push(value);
		this.#indices.set(value, index);
		return index;
	}

	encoded(): readonly Uint8Array[] {
		return this.#strings.map((value) => textEncoder.encode(value));
	}

	get length(): number {
		return this.#strings.length;
	}
}

function calculateFrameLength(
	strings: readonly Uint8Array[],
	frame: TerminalFrame,
): ProtocolResult<number> {
	let total = TERMINAL_FRAME_HEADER_BYTES;
	for (const string of strings) {
		const next = checkedAdd(total, 4, string.byteLength);
		if (!next.ok) return next;
		total = next.value;
	}
	const styleBytes = checkedMultiply(frame.styles.length, STYLE_RECORD_BYTES);
	if (!styleBytes.ok) return styleBytes;
	total += styleBytes.value;
	for (const row of frame.rowData) {
		const cellBytes = checkedMultiply(row.cells.length, CELL_RECORD_BYTES);
		if (!cellBytes.ok) return cellBytes;
		const next = checkedAdd(total, ROW_HEADER_BYTES, cellBytes.value);
		if (!next.ok) return next;
		total = next.value;
	}
	for (const extension of frame.extensions) {
		const next = checkedAdd(total, EXTENSION_HEADER_BYTES, extension.data.byteLength);
		if (!next.ok) return next;
		total = next.value;
	}
	return protocolSuccess(total);
}

export function encodeTerminalFrame(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = TerminalFrameSchema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Terminal frame validation failed: ${parsed.error.issues[0]?.message ?? 'unknown field error'}`,
		);
	}
	const frame = parsed.data;
	const stringTable = new StringTableBuilder();
	const titleIndex = stringTable.add(frame.title);
	const currentDirectoryIndex = stringTable.add(frame.currentDirectory);
	const hyperlinkIndices = frame.styles.map((style) => stringTable.add(style.hyperlink));
	const graphemeIndices = frame.rowData.map((row) =>
		row.cells.map((cell) => stringTable.add(cell.grapheme)),
	);
	if (stringTable.length > PROTOCOL_LIMITS.maxFrameStrings) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`Frame string table exceeds ${PROTOCOL_LIMITS.maxFrameStrings} entries`,
		);
	}
	const encodedStrings = stringTable.encoded();
	const totalLength = calculateFrameLength(encodedStrings, frame);
	if (!totalLength.ok) return totalLength;
	if (totalLength.value > PROTOCOL_LIMITS.maxFrameBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`Terminal frame exceeds ${PROTOCOL_LIMITS.maxFrameBytes} bytes`,
		);
	}

	const cellCount = frame.rowData.reduce((total, row) => total + row.cells.length, 0);
	const writer = new BinaryWriter(totalLength.value);
	writer.writeBytes(TERMINAL_FRAME_MAGIC);
	writer.writeUint8(TERMINAL_FRAME_FORMAT.major);
	writer.writeUint8(TERMINAL_FRAME_FORMAT.minor);
	writer.writeUint8(frameKindCode(frame.kind));
	writer.writeUint8(0);
	writer.writeUint32(totalLength.value);
	writer.writeUint16(TERMINAL_FRAME_HEADER_BYTES);
	writer.writeUint16(0);
	writer.writeBytes(identifierBytes(frame.hostEpoch));
	writer.writeBytes(identifierBytes(frame.attachmentId));
	writer.writeUint64(frame.sequence);
	writer.writeUint64(frame.baseRevision ?? MAX_UINT64);
	writer.writeUint64(frame.revision);
	writer.writeUint64(frame.generation);
	writer.writeUint64(frame.scrollbackRows);
	writer.writeUint16(frame.columns);
	writer.writeUint16(frame.rows);
	writer.writeUint16(frame.cursor.column);
	writer.writeUint16(frame.cursor.row);
	writer.writeUint8(cursorShapeCode(frame.cursor.shape));
	writer.writeUint8(
		(frame.cursor.visible ? 1 : 0) |
			(frame.cursor.blinking ? 1 << 1 : 0) |
			(frame.cursor.wideTail ? 1 << 2 : 0),
	);
	writer.writeUint8(screenCode(frame.activeScreen));
	writer.writeUint8(0);
	writer.writeUint32(interactionFlags(frame.interaction));
	writer.writeUint32(frame.defaultColors.foregroundRgb);
	writer.writeUint32(frame.defaultColors.backgroundRgb);
	writer.writeUint32(frame.defaultColors.cursorRgb ?? COLOR_DEFAULT);
	writer.writeUint32(titleIndex);
	writer.writeUint32(currentDirectoryIndex);
	writer.writeUint32(encodedStrings.length);
	writer.writeUint16(frame.styles.length);
	writer.writeUint16(frame.rowData.length);
	writer.writeUint16(frame.extensions.length);
	writer.writeUint16(0);
	writer.writeUint16(frame.cellPixelWidth);
	writer.writeUint16(frame.cellPixelHeight);
	writer.writeUint32(cellCount);
	writer.writeUint8(TERMINAL_FRAME_FORMAT.patch);
	writer.writeUint64(frame.themeRevision);
	writer.writeBytes(new Uint8Array(7));

	for (const string of encodedStrings) {
		writer.writeUint32(string.byteLength);
		writer.writeBytes(string);
	}
	for (const [index, style] of frame.styles.entries()) {
		writer.writeUint32(colorCode(style.foreground));
		writer.writeUint32(colorCode(style.background));
		writer.writeUint32(colorCode(style.underline));
		writer.writeUint32(hyperlinkIndices[index] ?? MAX_UINT32);
		writer.writeUint16(styleFlags(style));
		writer.writeUint8(underlineKindCode(style.underlineKind));
		writer.writeUint8(0);
	}
	for (const [rowOffset, row] of frame.rowData.entries()) {
		writer.writeUint16(row.index);
		writer.writeUint8((row.wrap ? 1 : 0) | (row.wrapContinuation ? 1 << 1 : 0));
		writer.writeUint8(promptClassCode(row.promptClass));
		writer.writeUint16(row.cells.length);
		writer.writeUint16(0);
		for (const [cellOffset, cell] of row.cells.entries()) {
			writer.writeUint32(graphemeIndices[rowOffset]?.[cellOffset] ?? MAX_UINT32);
			writer.writeUint16(cell.style);
			writer.writeUint8(
				cellWidthCode(cell.width) |
					(semanticContentCode(cell.semanticContent) << 2) |
					(cell.protected ? 1 << 4 : 0),
			);
			writer.writeUint8(0);
		}
	}
	for (const extension of frame.extensions) {
		writer.writeUint16(extension.kind);
		writer.writeUint16(extension.required ? 0x8000 : 0);
		writer.writeUint32(extension.data.byteLength);
		writer.writeBytes(extension.data);
	}
	return protocolSuccess(writer.bytes());
}

interface FrameHeader {
	readonly activeScreen: ActiveScreen;
	readonly attachmentId: AttachmentId;
	readonly baseRevision: bigint | null;
	readonly cellCount: number;
	readonly cellPixelHeight: number;
	readonly cellPixelWidth: number;
	readonly columns: number;
	readonly currentDirectoryIndex: number;
	readonly cursorBlinking: boolean;
	readonly cursorColumn: number;
	readonly cursorRow: number;
	readonly cursorShape: CursorShape;
	readonly cursorVisible: boolean;
	readonly cursorWideTail: boolean;
	readonly defaultBackgroundRgb: number;
	readonly defaultCursorRgb: number | null;
	readonly defaultForegroundRgb: number;
	readonly extensionCount: number;
	readonly generation: bigint;
	readonly hostEpoch: HostEpoch;
	readonly ignoredOptionalFlags: number;
	readonly interaction: TerminalInteraction;
	readonly kind: TerminalFrameKind;
	readonly revision: bigint;
	readonly rowCount: number;
	readonly rows: number;
	readonly scrollbackRows: bigint;
	readonly sequence: bigint;
	readonly stringCount: number;
	readonly styleCount: number;
	readonly themeRevision: bigint;
	readonly titleIndex: number;
}

function allZero(bytes: Uint8Array): boolean {
	return bytes.every((value) => value === 0);
}

function decodeFrameHeader(bytes: Uint8Array): ProtocolResult<FrameHeader> {
	if (bytes.byteLength < TERMINAL_FRAME_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.truncated,
			`Terminal frame requires at least ${TERMINAL_FRAME_HEADER_BYTES} bytes`,
		);
	}
	if (bytes.byteLength > PROTOCOL_LIMITS.maxFrameBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Terminal frame exceeds its limit');
	}
	if (!isMagic(bytes)) {
		return protocolFailure(ProtocolErrorCode.invalidMagic, 'Terminal frame magic is not OBTF');
	}

	const reader = new BinaryReader(bytes, TERMINAL_FRAME_MAGIC.byteLength);
	const formatMajor = reader.readUint8();
	const formatMinor = reader.readUint8();
	const kindCode = reader.readUint8();
	const flags = reader.readUint8();
	const totalLength = reader.readUint32();
	const headerLength = reader.readUint16();
	const reservedHeader = reader.readUint16();
	const hostEpochBytes = reader.readBytes(IDENTIFIER_BYTE_LENGTH);
	const attachmentIdBytes = reader.readBytes(IDENTIFIER_BYTE_LENGTH);
	const sequence = reader.readUint64();
	const encodedBaseRevision = reader.readUint64();
	const revision = reader.readUint64();
	const generation = reader.readUint64();
	const scrollbackRows = reader.readUint64();
	const columns = reader.readUint16();
	const rows = reader.readUint16();
	const cursorColumn = reader.readUint16();
	const cursorRow = reader.readUint16();
	const cursorShapeValue = reader.readUint8();
	const cursorFlags = reader.readUint8();
	const activeScreenCode = reader.readUint8();
	const reservedScreen = reader.readUint8();
	const encodedInteraction = reader.readUint32();
	const defaultForegroundRgb = reader.readUint32();
	const defaultBackgroundRgb = reader.readUint32();
	const encodedDefaultCursor = reader.readUint32();
	const titleIndex = reader.readUint32();
	const currentDirectoryIndex = reader.readUint32();
	const stringCount = reader.readUint32();
	const styleCount = reader.readUint16();
	const rowCount = reader.readUint16();
	const extensionCount = reader.readUint16();
	const reservedTables = reader.readUint16();
	const cellPixelWidth = reader.readUint16();
	const cellPixelHeight = reader.readUint16();
	const cellCount = reader.readUint32();
	const formatPatch = reader.readUint8();
	const themeRevision = reader.readUint64();
	const reservedTail = reader.readBytes(7);
	if (reader.failure) return reader.failure;

	if (
		formatMajor !== TERMINAL_FRAME_FORMAT.major ||
		formatMinor !== TERMINAL_FRAME_FORMAT.minor ||
		formatPatch !== TERMINAL_FRAME_FORMAT.patch ||
		headerLength !== TERMINAL_FRAME_HEADER_BYTES
	) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			`Unsupported terminal frame format ${formatMajor}.${formatMinor}.${formatPatch}`,
		);
	}
	if (totalLength !== bytes.byteLength) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Terminal frame length mismatch');
	}
	if (
		reservedHeader !== 0 ||
		reservedScreen !== 0 ||
		reservedTables !== 0 ||
		!allZero(reservedTail) ||
		(cursorFlags & ~CURSOR_FLAGS_MASK) !== 0 ||
		(flags & 0x80) !== 0
	) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Terminal frame contains a non-zero reserved or required flag',
		);
	}
	if (
		columns === 0 ||
		columns > PROTOCOL_LIMITS.maxFrameColumns ||
		rows === 0 ||
		rows > PROTOCOL_LIMITS.maxFrameRows ||
		columns * rows > PROTOCOL_LIMITS.maxFrameCells ||
		cursorColumn >= columns ||
		cursorRow >= rows ||
		(cellPixelWidth === 0) !== (cellPixelHeight === 0) ||
		cellPixelWidth > PROTOCOL_LIMITS.maxCellPixelDimension ||
		cellPixelHeight > PROTOCOL_LIMITS.maxCellPixelDimension ||
		columns * cellPixelWidth > MAX_UINT16 ||
		rows * cellPixelHeight > MAX_UINT16
	) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Terminal geometry is invalid');
	}
	if (
		stringCount > PROTOCOL_LIMITS.maxFrameStrings ||
		styleCount > PROTOCOL_LIMITS.maxFrameStyles ||
		rowCount > PROTOCOL_LIMITS.maxFrameRows ||
		cellCount > PROTOCOL_LIMITS.maxFrameCells ||
		extensionCount > PROTOCOL_LIMITS.maxFrameExtensions
	) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Terminal frame table exceeds a limit');
	}
	if (sequence === 0n || generation === 0n) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Sequence and generation must be positive',
		);
	}
	if (
		defaultForegroundRgb > COLOR_RGB_MASK ||
		defaultBackgroundRgb > COLOR_RGB_MASK ||
		(encodedDefaultCursor !== COLOR_DEFAULT && encodedDefaultCursor > COLOR_RGB_MASK)
	) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Default terminal color is invalid');
	}

	const kind = frameKindFromCode(kindCode);
	if (!kind.ok) return kind;
	const baseRevision = encodedBaseRevision === MAX_UINT64 ? null : encodedBaseRevision;
	if (
		(kind.value === TerminalFrameKind.snapshot && baseRevision !== null) ||
		(kind.value === TerminalFrameKind.delta &&
			(baseRevision === null || baseRevision >= revision)) ||
		(kind.value === TerminalFrameKind.snapshot &&
			(rowCount !== rows || cellCount !== rows * columns)) ||
		(kind.value === TerminalFrameKind.delta && cellCount !== rowCount * columns)
	) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Frame kind and base/table shape disagree',
		);
	}
	const shape = cursorShapeFromCode(cursorShapeValue);
	if (!shape.ok) return shape;
	const activeScreen = screenFromCode(activeScreenCode);
	if (!activeScreen.ok) return activeScreen;
	const interaction = interactionFromFlags(encodedInteraction);
	if (!interaction.ok) return interaction;
	const hostEpochText = identifierText(hostEpochBytes);
	if (!hostEpochText.ok) return hostEpochText;
	const hostEpoch = parseHostEpoch(hostEpochText.value);
	if (!hostEpoch.ok) return hostEpoch;
	const attachmentIdText = identifierText(attachmentIdBytes);
	if (!attachmentIdText.ok) return attachmentIdText;
	const attachmentId = parseAttachmentId(attachmentIdText.value);
	if (!attachmentId.ok) return attachmentId;

	return protocolSuccess({
		activeScreen: activeScreen.value,
		attachmentId: attachmentId.value,
		baseRevision,
		cellCount,
		cellPixelHeight,
		cellPixelWidth,
		columns,
		currentDirectoryIndex,
		cursorBlinking: (cursorFlags & (1 << 1)) !== 0,
		cursorColumn,
		cursorRow,
		cursorShape: shape.value,
		cursorVisible: (cursorFlags & 1) !== 0,
		cursorWideTail: (cursorFlags & (1 << 2)) !== 0,
		defaultBackgroundRgb,
		defaultCursorRgb: encodedDefaultCursor === COLOR_DEFAULT ? null : encodedDefaultCursor,
		defaultForegroundRgb,
		extensionCount,
		generation,
		hostEpoch: hostEpoch.value,
		ignoredOptionalFlags: flags & 0x7f,
		interaction: interaction.value,
		kind: kind.value,
		revision,
		rowCount,
		rows,
		scrollbackRows,
		sequence,
		stringCount,
		styleCount,
		themeRevision,
		titleIndex,
	});
}

function readStringTable(reader: BinaryReader, count: number): ProtocolResult<readonly string[]> {
	const strings: string[] = [];
	for (let index = 0; index < count; index += 1) {
		const length = reader.readUint32();
		if (reader.failure) return reader.failure;
		if (length > PROTOCOL_LIMITS.maxStringBytes) {
			return protocolFailure(ProtocolErrorCode.limitExceeded, `String ${index} exceeds its limit`);
		}
		const bytes = reader.readBytes(length);
		if (reader.failure) return reader.failure;
		try {
			strings.push(textDecoder.decode(bytes));
		} catch {
			return protocolFailure(ProtocolErrorCode.invalidUtf8, `String ${index} is not valid UTF-8`);
		}
	}
	return protocolSuccess(strings);
}

function readStyles(
	reader: BinaryReader,
	count: number,
	stringCount: number,
): ProtocolResult<DecodedTerminalStyles> {
	const foregroundColors = new Uint32Array(count);
	const backgroundColors = new Uint32Array(count);
	const underlineColors = new Uint32Array(count);
	const hyperlinkStringIndices = new Uint32Array(count);
	const flags = new Uint16Array(count);
	const underlineKinds = new Uint8Array(count);
	for (let index = 0; index < count; index += 1) {
		foregroundColors[index] = reader.readUint32();
		backgroundColors[index] = reader.readUint32();
		underlineColors[index] = reader.readUint32();
		const hyperlinkIndex = reader.readUint32();
		hyperlinkStringIndices[index] = hyperlinkIndex;
		flags[index] = reader.readUint16();
		underlineKinds[index] = reader.readUint8();
		const reserved = reader.readUint8();
		if (reader.failure) return reader.failure;
		if (reserved !== 0 || ((flags[index] ?? 0) & ~STYLE_FLAGS_MASK) !== 0) {
			return protocolFailure(
				ProtocolErrorCode.unknownRequiredFlag,
				`Style ${index} has unknown flags`,
			);
		}
		if ((underlineKinds[index] ?? 0) > 5) {
			return protocolFailure(ProtocolErrorCode.invalidField, `Style ${index} is invalid`);
		}
		if (
			!validColorCode(foregroundColors[index] ?? 0) ||
			!validColorCode(backgroundColors[index] ?? 0) ||
			!validColorCode(underlineColors[index] ?? 0) ||
			(hyperlinkIndex !== MAX_UINT32 && hyperlinkIndex >= stringCount)
		) {
			return protocolFailure(ProtocolErrorCode.invalidField, `Style ${index} is invalid`);
		}
	}
	return protocolSuccess({
		backgroundColors,
		flags,
		foregroundColors,
		hyperlinkStringIndices,
		underlineColors,
		underlineKinds,
	});
}

interface RowScan {
	readonly rows: DecodedTerminalRows;
	readonly rowsOffset: number;
}

function scanRows(
	reader: BinaryReader,
	bytes: Uint8Array,
	header: FrameHeader,
	stringCount: number,
): ProtocolResult<RowScan> {
	const rowsOffset = reader.offset;
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const indices = new Uint16Array(header.rowCount);
	const flags = new Uint8Array(header.rowCount);
	const promptClasses = new Uint8Array(header.rowCount);
	const cellOffsets = new Uint32Array(header.rowCount);
	let previousRow = -1;
	let previousRowFlags = 0;
	let previousSpacerHead = false;
	let totalCells = 0;

	for (let rowOffset = 0; rowOffset < header.rowCount; rowOffset += 1) {
		const row = reader.readUint16();
		const rowFlags = reader.readUint8();
		const promptClass = reader.readUint8();
		const cellCount = reader.readUint16();
		const reserved = reader.readUint16();
		if (reader.failure) return reader.failure;
		if (reserved !== 0 || (rowFlags & ~ROW_FLAGS_MASK) !== 0) {
			return protocolFailure(
				ProtocolErrorCode.unknownRequiredFlag,
				`Row ${rowOffset} has unknown flags`,
			);
		}
		if (
			promptClass > 2 ||
			cellCount !== header.columns ||
			row >= header.rows ||
			row <= previousRow ||
			(header.kind === TerminalFrameKind.snapshot && row !== rowOffset)
		) {
			return protocolFailure(ProtocolErrorCode.invalidField, `Row ${rowOffset} is invalid`);
		}
		const followsPriorRow = previousRow >= 0 && row === previousRow + 1;
		if (
			followsPriorRow &&
			((previousRowFlags & TerminalRowFlag.wrap) !== 0) !==
				((rowFlags & TerminalRowFlag.wrapContinuation) !== 0)
		) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Adjacent row wrap flags disagree');
		}
		indices[rowOffset] = row;
		flags[rowOffset] = rowFlags;
		promptClasses[rowOffset] = promptClass;
		cellOffsets[rowOffset] = totalCells;
		totalCells += cellCount;
		previousRow = row;

		let previousWidth = -1;
		let firstWidth = -1;
		let spacerHead = false;
		const cellRecordsOffset = reader.offset;
		reader.readBytes(cellCount * CELL_RECORD_BYTES);
		if (reader.failure) return reader.failure;
		for (let cellOffset = 0; cellOffset < cellCount; cellOffset += 1) {
			const recordOffset = cellRecordsOffset + cellOffset * CELL_RECORD_BYTES;
			const graphemeIndex = view.getUint32(recordOffset, true);
			const styleIndex = view.getUint16(recordOffset + 4, true);
			const cellFlags = view.getUint8(recordOffset + 6);
			const cellReserved = view.getUint8(recordOffset + 7);
			const width = cellFlags & 0b11;
			const semantic = (cellFlags >> 2) & 0b11;
			if (cellReserved !== 0 || (cellFlags & ~CELL_FLAGS_MASK) !== 0) {
				return protocolFailure(
					ProtocolErrorCode.unknownRequiredFlag,
					`Cell ${cellOffset} in row ${row} has unknown flags`,
				);
			}
			if (
				semantic > 2 ||
				graphemeIndex >= stringCount ||
				styleIndex >= header.styleCount ||
				(width === 2 && previousWidth !== 1) ||
				(previousWidth === 1 && width !== 2) ||
				(width === 3 && (cellOffset !== cellCount - 1 || (rowFlags & TerminalRowFlag.wrap) === 0))
			) {
				return protocolFailure(
					ProtocolErrorCode.invalidField,
					`Cell ${cellOffset} in row ${row} is invalid`,
				);
			}
			if (cellOffset === 0) firstWidth = width;
			if (width === 3) spacerHead = true;
			previousWidth = width;
		}
		if (previousWidth === 1) {
			return protocolFailure(ProtocolErrorCode.invalidField, `Row ${row} ends with a wide cell`);
		}
		if (followsPriorRow && previousSpacerHead && firstWidth !== 1) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'A row after spacer-head does not begin with a wide cell',
			);
		}
		previousRowFlags = rowFlags;
		previousSpacerHead = spacerHead;
	}
	if (totalCells !== header.cellCount) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'Row cell count disagrees with header',
		);
	}
	return protocolSuccess({ rows: { cellOffsets, flags, indices, promptClasses }, rowsOffset });
}

function readCells(
	bytes: Uint8Array,
	header: FrameHeader,
	scan: RowScan,
): ProtocolResult<DecodedTerminalCells> {
	const graphemeStringIndices = new Uint32Array(header.cellCount);
	const styleIndices = new Uint16Array(header.cellCount);
	const widths = new Uint8Array(header.cellCount);
	const semanticContents = new Uint8Array(header.cellCount);
	const protectedCells = new Uint8Array(header.cellCount);
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	let recordOffset = scan.rowsOffset;
	let cellOffset = 0;
	for (let rowOffset = 0; rowOffset < header.rowCount; rowOffset += 1) {
		recordOffset += ROW_HEADER_BYTES;
		for (let column = 0; column < header.columns; column += 1) {
			graphemeStringIndices[cellOffset] = view.getUint32(recordOffset, true);
			styleIndices[cellOffset] = view.getUint16(recordOffset + 4, true);
			const flags = view.getUint8(recordOffset + 6);
			widths[cellOffset] = flags & 0b11;
			semanticContents[cellOffset] = (flags >> 2) & 0b11;
			protectedCells[cellOffset] = (flags >> 4) & 1;
			recordOffset += CELL_RECORD_BYTES;
			cellOffset += 1;
		}
	}
	return protocolSuccess({
		graphemeStringIndices,
		protected: protectedCells,
		semanticContents,
		styleIndices,
		widths,
	});
}

function readExtensions(
	reader: BinaryReader,
	count: number,
): ProtocolResult<readonly TerminalFrameExtension[]> {
	const extensions: TerminalFrameExtension[] = [];
	let totalBytes = 0;
	for (let index = 0; index < count; index += 1) {
		const kind = reader.readUint16();
		const flags = reader.readUint16();
		const length = reader.readUint32();
		if (reader.failure) return reader.failure;
		if ((flags & 0x8000) !== 0) {
			return protocolFailure(
				ProtocolErrorCode.unknownRequiredFlag,
				`Frame extension ${kind} is required but unknown to format 0.1.0`,
			);
		}
		totalBytes += length;
		if (
			length > PROTOCOL_LIMITS.maxExtensionBytes ||
			totalBytes > PROTOCOL_LIMITS.maxExtensionBytes
		) {
			return protocolFailure(
				ProtocolErrorCode.limitExceeded,
				'Frame extensions exceed their limit',
			);
		}
		const data = new Uint8Array(reader.readBytes(length));
		if (reader.failure) return reader.failure;
		extensions.push({ data, kind, required: false });
	}
	return protocolSuccess(extensions);
}

function graphicsFromExtensions(
	extensions: readonly TerminalFrameExtension[],
	frameKind: TerminalFrameKind,
): ProtocolResult<TerminalGraphics | null> {
	const matches = extensions.filter(
		(extension) => extension.kind === TerminalFrameExtensionKind.kittyGraphics,
	);
	if (matches.length === 0) return protocolSuccess(null);
	if (matches.length !== 1) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal frame contains duplicate graphics extensions',
		);
	}
	const decoded = decodeTerminalGraphics(matches[0]?.data ?? new Uint8Array());
	if (!decoded.ok) return decoded;
	if (
		frameKind === TerminalFrameKind.snapshot &&
		decoded.value.kind !== TerminalGraphicsUpdateKind.snapshot
	) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal snapshot requires a graphics resource snapshot',
		);
	}
	return decoded;
}

function progressFromExtensions(
	extensions: readonly TerminalFrameExtension[],
): ProtocolResult<TerminalProgress> {
	const matches = extensions.filter(
		(extension) => extension.kind === TerminalFrameExtensionKind.progress,
	);
	if (matches.length === 0)
		return protocolSuccess({ state: TerminalProgressState.remove, value: null });
	if (matches.length !== 1) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal frame contains duplicate progress extensions',
		);
	}
	const data = matches[0]?.data;
	if (data === undefined || data.byteLength !== 2) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal progress extension is invalid',
		);
	}
	const state = (() => {
		switch (data[0]) {
			case TerminalProgressStateCode.remove:
				return TerminalProgressState.remove;
			case TerminalProgressStateCode.set:
				return TerminalProgressState.set;
			case TerminalProgressStateCode.error:
				return TerminalProgressState.error;
			case TerminalProgressStateCode.indeterminate:
				return TerminalProgressState.indeterminate;
			case TerminalProgressStateCode.pause:
				return TerminalProgressState.pause;
			default:
				return null;
		}
	})();
	if (state === null) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Terminal progress state is invalid');
	}
	const rawValue = data[1] ?? 0xff;
	const value = rawValue === 0xff ? null : rawValue;
	if (value !== null && value > 100) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Terminal progress exceeds 100 percent');
	}
	if (
		(state === TerminalProgressState.remove || state === TerminalProgressState.indeterminate) &&
		value !== null
	) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal progress state cannot carry a percentage',
		);
	}
	return protocolSuccess({ state, value });
}

function commandCompletionFromExtensions(
	extensions: readonly TerminalFrameExtension[],
): ProtocolResult<TerminalCommandCompletion | null> {
	const matches = extensions.filter(
		(extension) => extension.kind === TerminalFrameExtensionKind.commandCompletion,
	);
	if (matches.length === 0) return protocolSuccess(null);
	if (matches.length !== 1) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal frame contains duplicate command completion extensions',
		);
	}
	const data = matches[0]?.data;
	if (data === undefined || data.byteLength !== 25) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal command completion extension is invalid',
		);
	}
	const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
	const sequence = view.getBigUint64(0, true);
	const completedAtMilliseconds = view.getBigUint64(16, true);
	if (sequence === 0n || completedAtMilliseconds === 0n) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Terminal command completion identity and wall time must be positive',
		);
	}
	return protocolSuccess({
		completedAtMilliseconds,
		durationMilliseconds: view.getBigUint64(8, true),
		exitCode: view.getUint8(24),
		sequence,
	});
}

function resolveString(
	strings: readonly string[],
	index: number,
	field: string,
): ProtocolResult<string | null> {
	if (index === MAX_UINT32) return protocolSuccess(null);
	const value = strings[index];
	if (value === undefined) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`${field} references a missing string table entry`,
		);
	}
	return protocolSuccess(value);
}

export function decodeTerminalFrame(bytes: Uint8Array): ProtocolResult<DecodedTerminalFrame> {
	const header = decodeFrameHeader(bytes);
	if (!header.ok) return header;
	const reader = new BinaryReader(bytes, TERMINAL_FRAME_HEADER_BYTES);
	const strings = readStringTable(reader, header.value.stringCount);
	if (!strings.ok) return strings;
	const styles = readStyles(reader, header.value.styleCount, strings.value.length);
	if (!styles.ok) return styles;
	const scan = scanRows(reader, bytes, header.value, strings.value.length);
	if (!scan.ok) return scan;
	const extensions = readExtensions(reader, header.value.extensionCount);
	if (!extensions.ok) return extensions;
	const graphics = graphicsFromExtensions(extensions.value, header.value.kind);
	if (!graphics.ok) return graphics;
	const progress = progressFromExtensions(extensions.value);
	if (!progress.ok) return progress;
	const commandCompletion = commandCompletionFromExtensions(extensions.value);
	if (!commandCompletion.ok) return commandCompletion;
	if (reader.failure) return reader.failure;
	if (reader.offset !== bytes.byteLength) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Terminal frame has trailing bytes');
	}
	const cells = readCells(bytes, header.value, scan.value);
	if (!cells.ok) return cells;
	const title = resolveString(strings.value, header.value.titleIndex, 'Title');
	if (!title.ok) return title;
	const currentDirectory = resolveString(
		strings.value,
		header.value.currentDirectoryIndex,
		'Current directory',
	);
	if (!currentDirectory.ok) return currentDirectory;

	return protocolSuccess({
		activeScreen: header.value.activeScreen,
		attachmentId: header.value.attachmentId,
		baseRevision: header.value.baseRevision,
		cellPixelHeight: header.value.cellPixelHeight,
		cellPixelWidth: header.value.cellPixelWidth,
		cells: cells.value,
		columns: header.value.columns,
		commandCompletion: commandCompletion.value,
		currentDirectory: currentDirectory.value,
		cursor: {
			blinking: header.value.cursorBlinking,
			column: header.value.cursorColumn,
			row: header.value.cursorRow,
			shape: header.value.cursorShape,
			visible: header.value.cursorVisible,
			wideTail: header.value.cursorWideTail,
		},
		defaultColors: {
			backgroundRgb: header.value.defaultBackgroundRgb,
			cursorRgb: header.value.defaultCursorRgb,
			foregroundRgb: header.value.defaultForegroundRgb,
		},
		extensions: extensions.value,
		generation: header.value.generation,
		graphics: graphics.value,
		hostEpoch: header.value.hostEpoch,
		ignoredOptionalFlags: header.value.ignoredOptionalFlags,
		interaction: header.value.interaction,
		kind: header.value.kind,
		progress: progress.value,
		revision: header.value.revision,
		rowData: scan.value.rows,
		rows: header.value.rows,
		scrollbackRows: header.value.scrollbackRows,
		sequence: header.value.sequence,
		strings: strings.value,
		styles: styles.value,
		themeRevision: header.value.themeRevision,
		title: title.value,
	});
}

/**
 * Validates only the fixed header. Payload data is safe to consume only after `decodeTerminalFrame`
 * succeeds; this narrower result exists for routing and immutable duplicate recognition.
 */
export function inspectTerminalFrameHeader(
	bytes: Uint8Array,
): ProtocolResult<InspectedTerminalFrameHeader> {
	const header = decodeFrameHeader(bytes);
	return header.ok
		? protocolSuccess({
				attachmentId: header.value.attachmentId,
				baseRevision: header.value.baseRevision,
				generation: header.value.generation,
				hostEpoch: header.value.hostEpoch,
				kind: header.value.kind,
				revision: header.value.revision,
				sequence: header.value.sequence,
				themeRevision: header.value.themeRevision,
			})
		: header;
}

export function decodeTerminalFrameRoute(
	bytes: Uint8Array,
): ProtocolResult<DecodedTerminalFrameRoute> {
	const header = decodeFrameHeader(bytes);
	return header.ok
		? protocolSuccess({
				attachmentId: header.value.attachmentId,
				hostEpoch: header.value.hostEpoch,
			})
		: header;
}
