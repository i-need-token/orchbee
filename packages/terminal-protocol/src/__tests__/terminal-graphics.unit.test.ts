import { describe, expect, it } from 'vitest';

import { ProtocolErrorCode } from '../protocol-result';
import {
	decodeTerminalGraphics,
	encodeTerminalGraphics,
	TerminalGraphicsImageFormat,
	TerminalGraphicsUpdateKind,
	type TerminalGraphics,
} from '../terminal-graphics';

const snapshot: TerminalGraphics = {
	animationTick: 23n,
	images: [
		{
			data: new Uint8Array([255, 0, 0, 255, 0, 255, 0, 128]),
			format: TerminalGraphicsImageFormat.rgba,
			generation: 7n,
			height: 1,
			id: 11,
			width: 2,
		},
	],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [
		{
			column: -1,
			destinationHeight: 16,
			destinationWidth: 24,
			imageId: 11,
			internalId: false,
			offsetX: 2,
			offsetY: 3,
			placementId: 5,
			row: 2,
			sourceHeight: 1,
			sourceWidth: 2,
			sourceX: 0,
			sourceY: 0,
			z: 4,
		},
	],
	resourceRevision: 19n,
};

function encodeOrThrow(value: TerminalGraphics): Uint8Array {
	const encoded = encodeTerminalGraphics(value);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

describe('terminal graphics extension', () => {
	it('round-trips decoded pixels, crop geometry, and signed placement coordinates', () => {
		const decoded = decodeTerminalGraphics(encodeOrThrow(snapshot));
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value).toEqual(snapshot);
	});

	it('accepts placement-only updates without repeating image bytes', () => {
		const update: TerminalGraphics = {
			animationTick: snapshot.animationTick + 1n,
			images: [],
			kind: TerminalGraphicsUpdateKind.placements,
			placements: snapshot.placements,
			resourceRevision: snapshot.resourceRevision,
		};
		expect(decodeTerminalGraphics(encodeOrThrow(update))).toEqual({ ok: true, value: update });
	});

	it('rejects non-canonical metadata and unsafe resource geometry', () => {
		const placement = snapshot.placements[0];
		if (placement === undefined) throw new Error('Expected the canonical placement fixture.');
		expect(encodeTerminalGraphics({ ...snapshot, resourceRevision: 0n })).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(
			decodeTerminalGraphics(encodeOrThrow({ ...snapshot, placements: [{ ...placement, z: -1 }] })),
		).toEqual(
			expect.objectContaining({
				ok: true,
				value: expect.objectContaining({ placements: [{ ...placement, z: -1 }] }),
			}),
		);
		expect(
			encodeTerminalGraphics({
				...snapshot,
				placements: [{ ...placement, sourceWidth: 3 }],
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));

		const unknownFlags = encodeOrThrow(snapshot).slice();
		new DataView(unknownFlags.buffer).setUint32(unknownFlags.byteLength - 4, 2, true);
		expect(decodeTerminalGraphics(unknownFlags)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
		const mismatchedLength = encodeOrThrow(snapshot).slice();
		new DataView(mismatchedLength.buffer).setUint32(8, mismatchedLength.byteLength - 1, true);
		expect(decodeTerminalGraphics(mismatchedLength)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.lengthMismatch, ok: false }),
		);
	});

	it('rejects duplicate image identities atomically', () => {
		const image = snapshot.images[0];
		if (image === undefined) throw new Error('Expected the canonical image fixture.');
		const duplicate = {
			...snapshot,
			images: [image, image],
			placements: [],
		};
		expect(encodeTerminalGraphics(duplicate)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});
});
