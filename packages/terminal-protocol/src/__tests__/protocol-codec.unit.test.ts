import { readFile } from 'node:fs/promises';

import {
	array,
	assert as assertProperty,
	constantFrom,
	integer,
	property,
	uint8Array,
} from 'fast-check';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import { decodeControlJson, HelloSchema } from '../control';
import {
	decodeProtocolEnvelope,
	encodeProtocolEnvelope,
	MessageKind,
	PayloadEncoding,
} from '../envelope';
import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalFrame,
	encodeTerminalFrame,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	UnderlineKind,
} from '../frame';
import { parseHostEpoch } from '../identifiers';
import { decodeTerminalProjectionBuffer } from '../projection-buffer';
import { ProtocolErrorCode } from '../protocol-result';
import { decodeTerminalRowWindow, TerminalRowWindowDetail } from '../row-window';
import { CURRENT_PROTOCOL_VERSION } from '../version';

const hostEpoch = parseHostEpoch('11111111111111111111111111111111');
if (!hostEpoch.ok) throw new Error(hostEpoch.detail);

const frameFixtureUrl = new URL('../../fixtures/terminal-frame-v0.0.3.bin', import.meta.url);
const envelopeFixtureUrl = new URL(
	'../../fixtures/transport-envelope-v0.0.20.bin',
	import.meta.url,
);
const malformedFixtureUrl = new URL('../../fixtures/malformed-frame-v0.0.3.json', import.meta.url);
const rowWindowFixtureUrl = new URL(
	'../../fixtures/terminal-row-window-v0.0.3.bin',
	import.meta.url,
);
const malformedRowWindowFixtureUrl = new URL(
	'../../fixtures/malformed-row-window-v0.0.3.json',
	import.meta.url,
);
const packedProjectionConformanceFixtureUrl = new URL(
	'../../fixtures/packed-projection-conformance-v0.0.3.json',
	import.meta.url,
);

function emptyInteraction() {
	return {
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
}

const malformedCorpusSchema = z.strictObject({
	cases: z.array(
		z.strictObject({
			bytesBase64: z.string(),
			code: z.enum(ProtocolErrorCode),
			name: z.string(),
		}),
	),
});

const canonicalBase64Schema = z
	.string()
	.min(4)
	.refine(
		(value) =>
			/^(?:[A-Za-z\d+/]{4})*(?:[A-Za-z\d+/]{2}==|[A-Za-z\d+/]{3}=)?$/.test(value) &&
			Buffer.from(value, 'base64').toString('base64') === value,
		{ message: 'Expected non-empty canonical base64' },
	);
const corpusFailureSchema = z.strictObject({
	code: z.enum(ProtocolErrorCode),
	result: z.literal('failure'),
});
const projectionSuccessSchema = z.strictObject({
	observations: z.strictObject({
		ignoredOptionalFlags: z.number().int().min(0).max(0x7f),
		kind: z.enum(TerminalFrameKind),
		rowCount: z.number().int().min(0).max(4096),
		rowIndices: z.array(z.number().int().min(0).max(4095)),
	}),
	result: z.literal('success'),
});
const rowWindowSuccessSchema = z.strictObject({
	observations: z.strictObject({
		detail: z.enum(TerminalRowWindowDetail),
		ignoredOptionalFlags: z.number().int().min(0).max(0x7f),
		rowCount: z.number().int().min(1).max(4096),
	}),
	result: z.literal('success'),
});
const projectionConformanceCaseNames = [
	'obpb-width-out-of-range',
	'obpb-row-flag-unknown',
	'obpb-row-index-out-of-range',
	'obpb-prompt-class-out-of-range',
	'obpb-whole-string-invalid-utf8',
	'obpb-grapheme-length-splits-scalar',
	'obpb-grapheme-offset-null-nonnullable',
	'obpb-color-out-of-range',
	'obpb-style-flag-unknown',
	'obpb-underline-kind-out-of-range',
	'obpb-width-spacer-tail-without-head',
	'obpb-row-ends-with-wide-cell',
	'obpb-semantic-out-of-range',
	'obpb-protected-out-of-range',
	'obpb-adjacent-wrap-disagreement',
	'obpb-interaction-unknown-required-flag',
	'obpb-interaction-mouse-mode-conflict',
	'obpb-sparse-delta-increasing-row-indices',
	'obpb-sparse-delta-duplicate-row-indices',
	'obpb-sparse-delta-descending-row-indices',
	'obpb-optional-header-flag-01',
	'obpb-optional-header-flags-7f',
	'obpb-unknown-required-header-flag-80',
] as const;
const rowWindowConformanceCaseNames = [
	'obrw-optional-header-flag-01',
	'obrw-optional-header-flags-7f',
	'obrw-unknown-required-header-flag-80',
	'obrw-projection-section-length-mismatch',
	'obrw-graphics-section-length-mismatch',
	'obrw-graphics-update-not-snapshot',
] as const;
const packedProjectionConformanceCaseNames = [
	...projectionConformanceCaseNames,
	...rowWindowConformanceCaseNames,
] as const;
const packedProjectionCaseSchema = z.union([
	z.strictObject({
		bytesBase64: canonicalBase64Schema,
		codec: z.literal('OBPB'),
		expected: z.union([corpusFailureSchema, projectionSuccessSchema]),
		name: z.enum(projectionConformanceCaseNames),
		strictBoundary: z.literal('projection-buffer'),
	}),
	z.strictObject({
		bytesBase64: canonicalBase64Schema,
		codec: z.literal('OBRW'),
		expected: z.union([corpusFailureSchema, rowWindowSuccessSchema]),
		name: z.enum(rowWindowConformanceCaseNames),
		strictBoundary: z.literal('row-window'),
	}),
]);
const packedProjectionCorpusSchema = z
	.strictObject({
		cases: z.array(packedProjectionCaseSchema).length(29),
		formatVersion: z.literal('0.0.3'),
		schemaVersion: z.literal(1),
	})
	.superRefine((corpus, context) => {
		const names = new Set<string>();
		let projectionCases = 0;
		let rowWindowCases = 0;
		for (const [index, fixtureCase] of corpus.cases.entries()) {
			if (names.has(fixtureCase.name)) {
				context.addIssue({
					code: 'custom',
					message: `Duplicate conformance case name: ${fixtureCase.name}`,
					path: ['cases', index, 'name'],
				});
			}
			names.add(fixtureCase.name);
			if (fixtureCase.codec === 'OBPB') projectionCases += 1;
			else rowWindowCases += 1;
		}
		for (const requiredName of packedProjectionConformanceCaseNames) {
			if (!names.has(requiredName)) {
				context.addIssue({
					code: 'custom',
					message: `Missing conformance case: ${requiredName}`,
					path: ['cases'],
				});
			}
		}
		if (projectionCases !== 23) {
			context.addIssue({
				code: 'custom',
				message: `Expected 23 OBPB cases, received ${projectionCases}`,
				path: ['cases'],
			});
		}
		if (rowWindowCases !== 6) {
			context.addIssue({
				code: 'custom',
				message: `Expected 6 OBRW cases, received ${rowWindowCases}`,
				path: ['cases'],
			});
		}
	});

function projectionCorpusObservation(bytes: Uint8Array) {
	const decoded = decodeTerminalProjectionBuffer(bytes);
	if (!decoded.ok) return { code: decoded.code, result: 'failure' as const };
	return {
		observations: {
			ignoredOptionalFlags: decoded.value.ignoredOptionalFlags,
			kind: decoded.value.kind,
			rowCount: decoded.value.rowData.indices.length,
			rowIndices: [...decoded.value.rowData.indices],
		},
		result: 'success' as const,
	};
}

function rowWindowCorpusObservation(bytes: Uint8Array) {
	const decoded = decodeTerminalRowWindow(bytes);
	if (!decoded.ok) return { code: decoded.code, result: 'failure' as const };
	return {
		observations: {
			detail: decoded.value.detail,
			ignoredOptionalFlags: decoded.value.ignoredOptionalFlags,
			rowCount:
				decoded.value.detail === TerminalRowWindowDetail.render
					? decoded.value.frame.rows
					: decoded.value.rows.length,
		},
		result: 'success' as const,
	};
}

async function fixtureBytes(url: URL): Promise<Uint8Array> {
	return new Uint8Array(await readFile(url));
}

function locateFrameTables(bytes: Uint8Array): {
	readonly firstRow: number;
	readonly firstStyle: number;
} {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const stringCount = view.getUint32(124, true);
	const styleCount = view.getUint16(128, true);
	let offset = 160;
	for (let index = 0; index < stringCount; index += 1) {
		offset += 4 + view.getUint32(offset, true);
	}
	return { firstRow: offset + styleCount * 20, firstStyle: offset };
}

describe('terminal protocol identifiers and handshake input', () => {
	it('accepts only canonical non-zero 128-bit identifiers', () => {
		expect(parseHostEpoch('11111111111111111111111111111111').ok).toBe(true);
		expect(parseHostEpoch('11111111-1111-1111-1111-111111111111')).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidIdentifier, ok: false }),
		);
		expect(parseHostEpoch('00000000000000000000000000000000')).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidIdentifier, ok: false }),
		);
	});

	it('rejects malformed and oversized control JSON before it becomes trusted data', () => {
		expect(decodeControlJson(new TextEncoder().encode('{'), HelloSchema)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidJson, ok: false }),
		);
		expect(
			decodeControlJson(
				new TextEncoder().encode(
					JSON.stringify({
						clientId: '33333333333333333333333333333333',
						frameBytes: 1024,
						inputBytes: 1024,
						protocol: CURRENT_PROTOCOL_VERSION,
						rowWindowBytes: 1024,
						supportedOptionalFeatures: [],
					}),
				),
				HelloSchema,
			),
		).toEqual(
			expect.objectContaining({
				ok: true,
				value: expect.objectContaining({ protocol: CURRENT_PROTOCOL_VERSION }),
			}),
		);
	});
});

describe('transport envelope codec', () => {
	it('decodes the canonical envelope and preserves its packed frame payload', async () => {
		const envelopeBytes = await fixtureBytes(envelopeFixtureUrl);
		const frameBytes = await fixtureBytes(frameFixtureUrl);
		const decoded = decodeProtocolEnvelope(envelopeBytes);
		if (!decoded.ok) throw new Error(decoded.detail);

		expect(decoded.value).toEqual(
			expect.objectContaining({
				event: true,
				hostEpoch: hostEpoch.value,
				kind: MessageKind.deltaFrame,
				knownMessage: true,
				payloadEncoding: PayloadEncoding.terminalFrame,
				protocol: CURRENT_PROTOCOL_VERSION,
			}),
		);
		expect(decoded.value.payload).toEqual(frameBytes);
	});

	it('skips unknown optional messages and rejects unknown required messages or flags', async () => {
		const optional = (await fixtureBytes(envelopeFixtureUrl)).slice();
		const optionalView = new DataView(optional.buffer);
		optionalView.setUint16(6, optionalView.getUint16(6, true) | 1, true);
		optionalView.setUint16(20, 60_000, true);
		expect(decodeProtocolEnvelope(optional)).toEqual(
			expect.objectContaining({
				ok: true,
				value: expect.objectContaining({ knownMessage: false, optionalMessage: true }),
			}),
		);

		const requiredMessage = optional.slice();
		new DataView(requiredMessage.buffer).setUint16(6, 0, true);
		expect(decodeProtocolEnvelope(requiredMessage)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredMessage, ok: false }),
		);

		const requiredFlag = optional.slice();
		new DataView(requiredFlag.buffer).setUint16(6, 0x0101, true);
		expect(decodeProtocolEnvelope(requiredFlag)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
	});

	it('round-trips bounded binary payloads under a stable property seed', () => {
		expect(() =>
			assertProperty(
				property(
					uint8Array({ maxLength: 512 }),
					constantFrom(
						{
							kind: MessageKind.writeInput,
							payloadEncoding: PayloadEncoding.rawBytes,
							role: 'request',
						},
						{
							kind: MessageKind.inputAccepted,
							payloadEncoding: PayloadEncoding.controlJson,
							role: 'response',
						},
						{
							kind: MessageKind.selectionExportChunk,
							payloadEncoding: PayloadEncoding.rawBytes,
							role: 'event',
						},
					),
					(payload, contract) => {
						const encoded = encodeProtocolEnvelope({
							correlationId: null,
							event: contract.role === 'event',
							hostEpoch: hostEpoch.value,
							kind: contract.kind,
							optionalMessage: false,
							payload,
							payloadEncoding: contract.payloadEncoding,
							protocol: CURRENT_PROTOCOL_VERSION,
							response: contract.role === 'response',
						});
						if (!encoded.ok) return false;
						const decoded = decodeProtocolEnvelope(encoded.value);
						return (
							decoded.ok && decoded.value.payload.every((byte, index) => byte === payload[index])
						);
					},
				),
				{ numRuns: 200, seed: 0x0b7bee },
			),
		).not.toThrow();
	});
});

describe('terminal frame codec', () => {
	it('decodes canonical Unicode, styles, cursor, extension, and typed cell columns', async () => {
		const frameBytes = await fixtureBytes(frameFixtureUrl);
		const decoded = decodeTerminalFrame(frameBytes);
		if (!decoded.ok) throw new Error(decoded.detail);

		expect(decoded.value).toEqual(
			expect.objectContaining({
				attachmentId: '22222222222222222222222222222222',
				baseRevision: 41n,
				columns: 6,
				currentDirectory: '/tmp/项目',
				hostEpoch: hostEpoch.value,
				kind: TerminalFrameKind.delta,
				revision: 42n,
				rows: 2,
				sequence: 7n,
				themeRevision: 5n,
				title: 'Orchbee 🐝',
			}),
		);
		expect(decoded.value.cursor).toEqual({
			blinking: true,
			column: 4,
			row: 1,
			shape: CursorShape.bar,
			visible: true,
			wideTail: false,
		});
		expect(decoded.value.cells.graphemeStringIndices).toBeInstanceOf(Uint32Array);
		expect(decoded.value.cells.styleIndices).toBeInstanceOf(Uint16Array);
		expect([...decoded.value.cells.widths]).toEqual([0, 1, 2, 0, 0, 3, 1, 2, 0, 0, 0, 0]);
		expect(decoded.value.strings).toEqual(
			expect.arrayContaining(['界', 'é', '👩🏽‍💻', 'https://orchbee.dev/终端']),
		);
		expect(decoded.value.extensions).toEqual([
			{ data: new Uint8Array([1, 2, 3, 4]), kind: 32_769, required: false },
		]);
	});

	it('rejects every tracked malformed frame with its stable error class', async () => {
		const raw = await readFile(malformedFixtureUrl, 'utf8');
		const corpus = malformedCorpusSchema.parse(JSON.parse(raw));
		for (const fixture of corpus.cases) {
			const decoded = decodeTerminalFrame(
				new Uint8Array(Buffer.from(fixture.bytesBase64, 'base64')),
			);
			expect({ name: fixture.name, result: decoded }).toEqual({
				name: fixture.name,
				result: expect.objectContaining({ code: fixture.code, ok: false }),
			});
		}
	});

	it('rejects non-canonical style, wrap, and spacer-head encodings', async () => {
		const canonical = await fixtureBytes(frameFixtureUrl);
		const tables = locateFrameTables(canonical);

		const invalidUnderline = canonical.slice();
		invalidUnderline[tables.firstStyle + 18] = 6;
		expect(decodeTerminalFrame(invalidUnderline)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const invalidSpacerHead = canonical.slice();
		const nonFinalCellFlags = tables.firstRow + 8 + 4 * 8 + 6;
		invalidSpacerHead[nonFinalCellFlags] =
			((invalidSpacerHead[nonFinalCellFlags] ?? 0) & ~0b11) | 0b11;
		expect(decodeTerminalFrame(invalidSpacerHead)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const invalidWrapContinuation = canonical.slice();
		const columns = new DataView(canonical.buffer, canonical.byteOffset).getUint16(88, true);
		const secondRow = tables.firstRow + 8 + columns * 8;
		invalidWrapContinuation[secondRow + 2] = (invalidWrapContinuation[secondRow + 2] ?? 0) & ~0b10;
		expect(decodeTerminalFrame(invalidWrapContinuation)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('rejects a frame patch outside the exact current format', async () => {
		const differentPatch = (await fixtureBytes(frameFixtureUrl)).slice();
		differentPatch[144] = 4;
		expect(decodeTerminalFrame(differentPatch)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unsupportedVersion, ok: false }),
		);
	});

	it('round-trips generated bounded rows without per-cell decoded objects', () => {
		const graphemes = array(constantFrom('a', '界', 'é', '👩🏽‍💻', ''), {
			maxLength: 20,
			minLength: 1,
		});
		expect(() =>
			assertProperty(
				property(graphemes, (values) => {
					const encoded = encodeTerminalFrame({
						activeScreen: ActiveScreen.primary,
						attachmentId: '22222222222222222222222222222222',
						baseRevision: null,
						cellPixelHeight: 16,
						cellPixelWidth: 8,
						columns: values.length,
						cursor: {
							blinking: false,
							column: 0,
							row: 0,
							shape: CursorShape.block,
							visible: true,
							wideTail: false,
						},
						currentDirectory: null,
						defaultColors: {
							backgroundRgb: 0,
							cursorRgb: null,
							foregroundRgb: 0xffffff,
						},
						extensions: [],
						generation: 1n,
						hostEpoch: hostEpoch.value,
						interaction: emptyInteraction(),
						kind: TerminalFrameKind.snapshot,
						revision: 0n,
						rowData: [
							{
								cells: values.map((grapheme) => ({
									grapheme,
									protected: false,
									semanticContent: SemanticContent.output,
									style: 0,
									width: CellWidth.narrow,
								})),
								index: 0,
								promptClass: RowPromptClass.none,
								wrap: false,
								wrapContinuation: false,
							},
						],
						rows: 1,
						scrollbackRows: 0n,
						sequence: 1n,
						styles: [
							{
								background: { kind: TerminalColorKind.default },
								blink: false,
								bold: false,
								faint: false,
								foreground: { kind: TerminalColorKind.default },
								hyperlink: null,
								inverse: false,
								invisible: false,
								italic: false,
								overline: false,
								strikethrough: false,
								underline: { kind: TerminalColorKind.none },
								underlineKind: UnderlineKind.none,
							},
						],
						themeRevision: 0n,
						title: null,
					});
					if (!encoded.ok) return false;
					const decoded = decodeTerminalFrame(encoded.value);
					return decoded.ok && decoded.value.cells.widths.length === values.length;
				}),
				{ numRuns: 200, seed: 0x7f4a6e },
			),
		).not.toThrow();
	});

	it('rejects every strict prefix of the canonical frame without throwing', async () => {
		const frameBytes = await fixtureBytes(frameFixtureUrl);
		expect(() =>
			assertProperty(
				property(integer({ max: frameBytes.length - 1, min: 0 }), (length) => {
					return !decodeTerminalFrame(frameBytes.subarray(0, length)).ok;
				}),
				{ numRuns: 200, seed: 0x5eeda1 },
			),
		).not.toThrow();
	});
});

describe('terminal row-window codec', () => {
	it('decodes the canonical Unicode text window and typed mappings', async () => {
		const decoded = decodeTerminalRowWindow(await fixtureBytes(rowWindowFixtureUrl));
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.detail).toBe(TerminalRowWindowDetail.text);
		if (decoded.value.detail !== TerminalRowWindowDetail.text) return;
		expect(decoded.value).toEqual(
			expect.objectContaining({
				columns: 6,
				generation: 3n,
				revision: 42n,
				sequence: 8n,
				startRow: 100n,
				totalRows: 125n,
			}),
		);
		expect(decoded.value.rows.map((row) => row.text)).toEqual(['A界é ', '👩🏽‍💻B']);
		expect([...(decoded.value.rows[1]?.utf16Offsets ?? [])]).toEqual([0, 7]);
	});

	it('rejects every tracked malformed row window with its stable error class', async () => {
		const raw = await readFile(malformedRowWindowFixtureUrl, 'utf8');
		const corpus = malformedCorpusSchema.parse(JSON.parse(raw));
		for (const fixture of corpus.cases) {
			const decoded = decodeTerminalRowWindow(
				new Uint8Array(Buffer.from(fixture.bytesBase64, 'base64')),
			);
			expect({ name: fixture.name, result: decoded }).toEqual({
				name: fixture.name,
				result: expect.objectContaining({ code: fixture.code, ok: false }),
			});
		}
	});

	it('rejects non-canonical text mapping, prompt, wrap, and patch fields', async () => {
		const canonical = await fixtureBytes(rowWindowFixtureUrl);
		const view = new DataView(canonical.buffer, canonical.byteOffset, canonical.byteLength);
		const firstTextLength = view.getUint32(116, true);
		const firstMappingCount = view.getUint32(120, true);
		const firstMapping = 124 + firstTextLength;
		const secondMapping = firstMapping + 12;
		const secondRow = firstMapping + firstMappingCount * 12;

		const mismatchedUnicodeOffsets = canonical.slice();
		new DataView(mismatchedUnicodeOffsets.buffer).setUint32(secondMapping, 2, true);
		expect(decodeTerminalRowWindow(mismatchedUnicodeOffsets)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const overlappingColumns = canonical.slice();
		overlappingColumns[firstMapping + 10] = 2;
		expect(decodeTerminalRowWindow(overlappingColumns)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const invalidPrompt = canonical.slice();
		invalidPrompt[113] = 3;
		expect(decodeTerminalRowWindow(invalidPrompt)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const invalidWrapContinuation = canonical.slice();
		invalidWrapContinuation[secondRow] = (invalidWrapContinuation[secondRow] ?? 0) & ~0b10;
		expect(decodeTerminalRowWindow(invalidWrapContinuation)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const differentPatch = canonical.slice();
		differentPatch[100] = 4;
		expect(decodeTerminalRowWindow(differentPatch)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unsupportedVersion, ok: false }),
		);
	});
});

describe('packed projection cross-language corpus', () => {
	it('rejects malformed case identity, routing, bytes, outcomes, observations, and inventory', async () => {
		const raw = await readFile(packedProjectionConformanceFixtureUrl, 'utf8');
		const corpus = packedProjectionCorpusSchema.parse(JSON.parse(raw));
		const first = corpus.cases[0];
		const second = corpus.cases[1];
		const successCase = corpus.cases.find(
			(fixtureCase) => fixtureCase.codec === 'OBPB' && fixtureCase.expected.result === 'success',
		);
		if (
			first === undefined ||
			second === undefined ||
			first.codec !== second.codec ||
			successCase === undefined ||
			successCase.codec !== 'OBPB' ||
			successCase.expected.result !== 'success'
		) {
			throw new Error('Packed projection schema fixture is incomplete');
		}
		const replaceFirst = (replacement: unknown) => ({
			...corpus,
			cases: [replacement, ...corpus.cases.slice(1)],
		});
		const malformedSuccess = {
			...successCase,
			expected: {
				observations: {
					ignoredOptionalFlags: successCase.expected.observations.ignoredOptionalFlags,
					kind: successCase.expected.observations.kind,
					rowCount: successCase.expected.observations.rowCount,
				},
				result: 'success',
			},
		};
		const corruptions = [
			{ name: 'name', value: replaceFirst({ ...first, name: 'OBPB invalid name' }) },
			{
				name: 'unknown well-formed case',
				value: replaceFirst({ ...first, name: 'obpb-unrecognized-but-valid' }),
			},
			{ name: 'duplicate case', value: replaceFirst({ ...first, name: second.name }) },
			{ name: 'codec', value: replaceFirst({ ...first, codec: 'OBPX' }) },
			{ name: 'base64', value: replaceFirst({ ...first, bytesBase64: '***' }) },
			{
				name: 'outcome',
				value: replaceFirst({ ...first, expected: { result: 'ignored' } }),
			},
			{
				name: 'success observations',
				value: {
					...corpus,
					cases: corpus.cases.map((fixtureCase) =>
						fixtureCase.name === successCase.name ? malformedSuccess : fixtureCase,
					),
				},
			},
			{ name: 'missing case', value: { ...corpus, cases: corpus.cases.slice(1) } },
		];
		for (const corruption of corruptions) {
			expect({
				name: corruption.name,
				success: packedProjectionCorpusSchema.safeParse(corruption.value).success,
			}).toEqual({ name: corruption.name, success: false });
		}
		expect(
			packedProjectionCorpusSchema.safeParse({
				...corpus,
				cases: [...corpus.cases.slice(1), first],
			}).success,
		).toBe(true);
	});

	it('consumes every authored OBPB and OBRW case through its strict TypeScript decoder', async () => {
		const raw = await readFile(packedProjectionConformanceFixtureUrl, 'utf8');
		const corpus = packedProjectionCorpusSchema.parse(JSON.parse(raw));
		for (const fixtureCase of corpus.cases) {
			const bytes = new Uint8Array(Buffer.from(fixtureCase.bytesBase64, 'base64'));
			const observed =
				fixtureCase.codec === 'OBPB'
					? projectionCorpusObservation(bytes)
					: rowWindowCorpusObservation(bytes);
			expect({ name: fixtureCase.name, observed }).toEqual({
				name: fixtureCase.name,
				observed: fixtureCase.expected,
			});
		}
	});
});
