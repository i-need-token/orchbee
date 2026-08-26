import { BinaryReader, BinaryWriter, checkedAdd, checkedMultiply } from './binary';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';

export const TERMINAL_GRAPHICS_MAGIC = new Uint8Array([0x4f, 0x42, 0x4b, 0x47]);
export const TERMINAL_GRAPHICS_FORMAT = { major: 0, minor: 0, patch: 2 } as const;
export const TERMINAL_GRAPHICS_HEADER_BYTES = 40;

const IMAGE_RECORD_BYTES = 32;
const PLACEMENT_RECORD_BYTES = 56;
const MAX_UINT32 = 0xffff_ffff;
const MAX_UINT64 = 0xffff_ffff_ffff_ffffn;
const PLACEMENT_FLAG_INTERNAL_ID = 1;

export const TerminalGraphicsUpdateKind = {
	placements: 'placements',
	snapshot: 'snapshot',
} as const;

export const TerminalGraphicsImageFormat = {
	gray: 'gray',
	grayAlpha: 'gray-alpha',
	rgb: 'rgb',
	rgba: 'rgba',
} as const;

export type TerminalGraphicsUpdateKind =
	(typeof TerminalGraphicsUpdateKind)[keyof typeof TerminalGraphicsUpdateKind];
export type TerminalGraphicsImageFormat =
	(typeof TerminalGraphicsImageFormat)[keyof typeof TerminalGraphicsImageFormat];

export interface TerminalGraphicsImage {
	readonly data: Uint8Array;
	readonly format: TerminalGraphicsImageFormat;
	readonly generation: bigint;
	readonly height: number;
	readonly id: number;
	readonly width: number;
}

export interface TerminalGraphicsPlacement {
	readonly column: number;
	readonly destinationHeight: number;
	readonly destinationWidth: number;
	readonly imageId: number;
	readonly internalId: boolean;
	readonly offsetX: number;
	readonly offsetY: number;
	readonly placementId: number;
	readonly row: number;
	readonly sourceHeight: number;
	readonly sourceWidth: number;
	readonly sourceX: number;
	readonly sourceY: number;
	readonly z: number;
}

export interface TerminalGraphics {
	readonly animationTick: bigint;
	readonly images: readonly TerminalGraphicsImage[];
	readonly kind: TerminalGraphicsUpdateKind;
	readonly placements: readonly TerminalGraphicsPlacement[];
	readonly resourceRevision: bigint;
}

function isMagic(bytes: Uint8Array): boolean {
	return TERMINAL_GRAPHICS_MAGIC.every((value, index) => bytes[index] === value);
}

function formatCode(format: TerminalGraphicsImageFormat): number {
	switch (format) {
		case TerminalGraphicsImageFormat.gray:
			return 1;
		case TerminalGraphicsImageFormat.grayAlpha:
			return 2;
		case TerminalGraphicsImageFormat.rgb:
			return 3;
		case TerminalGraphicsImageFormat.rgba:
			return 4;
	}
}

function formatFromCode(code: number): ProtocolResult<TerminalGraphicsImageFormat> {
	switch (code) {
		case 1:
			return protocolSuccess(TerminalGraphicsImageFormat.gray);
		case 2:
			return protocolSuccess(TerminalGraphicsImageFormat.grayAlpha);
		case 3:
			return protocolSuccess(TerminalGraphicsImageFormat.rgb);
		case 4:
			return protocolSuccess(TerminalGraphicsImageFormat.rgba);
		default:
			return protocolFailure(ProtocolErrorCode.invalidField, `Unknown graphics format ${code}`);
	}
}

function bytesPerPixel(format: TerminalGraphicsImageFormat): number {
	switch (format) {
		case TerminalGraphicsImageFormat.gray:
			return 1;
		case TerminalGraphicsImageFormat.grayAlpha:
			return 2;
		case TerminalGraphicsImageFormat.rgb:
			return 3;
		case TerminalGraphicsImageFormat.rgba:
			return 4;
	}
}

function updateKindCode(kind: TerminalGraphicsUpdateKind): number {
	return kind === TerminalGraphicsUpdateKind.snapshot ? 1 : 2;
}

function updateKindFromCode(code: number): ProtocolResult<TerminalGraphicsUpdateKind> {
	if (code === 1) return protocolSuccess(TerminalGraphicsUpdateKind.snapshot);
	if (code === 2) return protocolSuccess(TerminalGraphicsUpdateKind.placements);
	return protocolFailure(ProtocolErrorCode.invalidField, `Unknown graphics update kind ${code}`);
}

function signedInt32(value: number): boolean {
	return Number.isInteger(value) && value >= -0x8000_0000 && value <= 0x7fff_ffff;
}

function unsignedInt32(value: number): boolean {
	return Number.isInteger(value) && value >= 0 && value <= MAX_UINT32;
}

function encodeInt32(value: number): number {
	return value >>> 0;
}

function decodeInt32(value: number): number {
	return value > 0x7fff_ffff ? value - 0x1_0000_0000 : value;
}

function pixelByteLength(image: TerminalGraphicsImage): ProtocolResult<number> {
	const pixels = checkedMultiply(image.width, image.height);
	return pixels.ok ? checkedMultiply(pixels.value, bytesPerPixel(image.format)) : pixels;
}

function validImageShape(image: TerminalGraphicsImage): ProtocolResult<number> {
	if (
		!unsignedInt32(image.id) ||
		image.id === 0 ||
		image.generation <= 0n ||
		image.generation > MAX_UINT64 ||
		!Number.isInteger(image.width) ||
		!Number.isInteger(image.height) ||
		image.width <= 0 ||
		image.height <= 0 ||
		image.width > PROTOCOL_LIMITS.maxTerminalGraphicsDimension ||
		image.height > PROTOCOL_LIMITS.maxTerminalGraphicsDimension
	) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics image metadata is invalid');
	}
	const expected = pixelByteLength(image);
	if (!expected.ok) return expected;
	if (expected.value !== image.data.byteLength) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Graphics pixel length is invalid');
	}
	return expected;
}

function validPlacementShape(placement: TerminalGraphicsPlacement): boolean {
	return (
		unsignedInt32(placement.imageId) &&
		placement.imageId !== 0 &&
		unsignedInt32(placement.placementId) &&
		signedInt32(placement.column) &&
		signedInt32(placement.row) &&
		unsignedInt32(placement.offsetX) &&
		unsignedInt32(placement.offsetY) &&
		unsignedInt32(placement.sourceX) &&
		unsignedInt32(placement.sourceY) &&
		Number.isInteger(placement.sourceWidth) &&
		placement.sourceWidth > 0 &&
		placement.sourceWidth <= PROTOCOL_LIMITS.maxTerminalGraphicsDimension &&
		Number.isInteger(placement.sourceHeight) &&
		placement.sourceHeight > 0 &&
		placement.sourceHeight <= PROTOCOL_LIMITS.maxTerminalGraphicsDimension &&
		Number.isInteger(placement.destinationWidth) &&
		placement.destinationWidth > 0 &&
		placement.destinationWidth <= PROTOCOL_LIMITS.maxTerminalGraphicsDestinationPixels &&
		Number.isInteger(placement.destinationHeight) &&
		placement.destinationHeight > 0 &&
		placement.destinationHeight <= PROTOCOL_LIMITS.maxTerminalGraphicsDestinationPixels &&
		signedInt32(placement.z)
	);
}

function validateGraphics(value: TerminalGraphics): ProtocolResult<number> {
	if (
		value.animationTick < 0n ||
		value.animationTick > MAX_UINT64 ||
		value.resourceRevision < 0n ||
		value.resourceRevision > MAX_UINT64 ||
		value.images.length > PROTOCOL_LIMITS.maxTerminalGraphicsImages ||
		value.placements.length > PROTOCOL_LIMITS.maxTerminalGraphicsPlacements ||
		(value.kind === TerminalGraphicsUpdateKind.placements && value.images.length !== 0)
	) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Graphics table exceeds its limit');
	}

	let imageBytes = 0;
	const imageIds = new Set<number>();
	for (const image of value.images) {
		const length = validImageShape(image);
		if (!length.ok) return length;
		if (imageIds.has(image.id)) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics image IDs must be unique');
		}
		imageIds.add(image.id);
		imageBytes += length.value;
		if (imageBytes > PROTOCOL_LIMITS.maxTerminalGraphicsResourceBytes) {
			return protocolFailure(ProtocolErrorCode.limitExceeded, 'Graphics pixels exceed their limit');
		}
	}
	if (value.images.length > 0 && value.resourceRevision === 0n) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics resources require a revision');
	}
	for (const placement of value.placements) {
		if (!validPlacementShape(placement)) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics placement is invalid');
		}
		if (value.kind === TerminalGraphicsUpdateKind.snapshot && !imageIds.has(placement.imageId)) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Graphics placement references no image',
			);
		}
		const image = value.images.find((candidate) => candidate.id === placement.imageId);
		if (
			image !== undefined &&
			(placement.sourceX + placement.sourceWidth > image.width ||
				placement.sourceY + placement.sourceHeight > image.height)
		) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics crop exceeds its image');
		}
	}
	const records = checkedAdd(
		TERMINAL_GRAPHICS_HEADER_BYTES,
		value.images.length * IMAGE_RECORD_BYTES,
		value.placements.length * PLACEMENT_RECORD_BYTES,
		imageBytes,
	);
	if (!records.ok) return records;
	if (records.value > PROTOCOL_LIMITS.maxTerminalGraphicsExtensionBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Graphics extension exceeds its limit');
	}
	return records;
}

export function encodeTerminalGraphics(value: TerminalGraphics): ProtocolResult<Uint8Array> {
	const length = validateGraphics(value);
	if (!length.ok) return length;
	const imageByteLength = value.images.reduce((total, image) => total + image.data.byteLength, 0);
	const writer = new BinaryWriter(length.value);
	writer.writeBytes(TERMINAL_GRAPHICS_MAGIC);
	writer.writeUint8(TERMINAL_GRAPHICS_FORMAT.major);
	writer.writeUint8(TERMINAL_GRAPHICS_FORMAT.minor);
	writer.writeUint8(TERMINAL_GRAPHICS_FORMAT.patch);
	writer.writeUint8(updateKindCode(value.kind));
	writer.writeUint32(length.value);
	writer.writeUint64(value.resourceRevision);
	writer.writeUint64(value.animationTick);
	writer.writeUint16(value.images.length);
	writer.writeUint16(value.placements.length);
	writer.writeUint32(imageByteLength);
	writer.writeUint32(0);
	for (const image of value.images) {
		writer.writeUint32(image.id);
		writer.writeUint8(formatCode(image.format));
		writer.writeBytes(new Uint8Array(3));
		writer.writeUint64(image.generation);
		writer.writeUint32(image.width);
		writer.writeUint32(image.height);
		writer.writeUint32(image.data.byteLength);
		writer.writeUint32(0);
		writer.writeBytes(image.data);
	}
	for (const placement of value.placements) {
		writer.writeUint32(placement.imageId);
		writer.writeUint32(placement.placementId);
		writer.writeUint32(encodeInt32(placement.column));
		writer.writeUint32(encodeInt32(placement.row));
		writer.writeUint32(placement.offsetX);
		writer.writeUint32(placement.offsetY);
		writer.writeUint32(placement.sourceX);
		writer.writeUint32(placement.sourceY);
		writer.writeUint32(placement.sourceWidth);
		writer.writeUint32(placement.sourceHeight);
		writer.writeUint32(placement.destinationWidth);
		writer.writeUint32(placement.destinationHeight);
		writer.writeUint32(encodeInt32(placement.z));
		writer.writeUint32(placement.internalId ? PLACEMENT_FLAG_INTERNAL_ID : 0);
	}
	return protocolSuccess(writer.bytes());
}

export function decodeTerminalGraphics(bytes: Uint8Array): ProtocolResult<TerminalGraphics> {
	if (bytes.byteLength < TERMINAL_GRAPHICS_HEADER_BYTES) {
		return protocolFailure(ProtocolErrorCode.truncated, 'Graphics extension header is truncated');
	}
	if (bytes.byteLength > PROTOCOL_LIMITS.maxTerminalGraphicsExtensionBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Graphics extension exceeds its limit');
	}
	if (!isMagic(bytes)) {
		return protocolFailure(ProtocolErrorCode.invalidMagic, 'Graphics extension magic is not OBKG');
	}
	const reader = new BinaryReader(bytes, TERMINAL_GRAPHICS_MAGIC.byteLength);
	const major = reader.readUint8();
	const minor = reader.readUint8();
	const patch = reader.readUint8();
	const kindCode = reader.readUint8();
	const totalLength = reader.readUint32();
	const resourceRevision = reader.readUint64();
	const animationTick = reader.readUint64();
	const imageCount = reader.readUint16();
	const placementCount = reader.readUint16();
	const declaredImageBytes = reader.readUint32();
	const reservedHeader = reader.readUint32();
	if (reader.failure) return reader.failure;
	if (
		major !== TERMINAL_GRAPHICS_FORMAT.major ||
		minor !== TERMINAL_GRAPHICS_FORMAT.minor ||
		patch !== TERMINAL_GRAPHICS_FORMAT.patch
	) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			`Unsupported graphics extension format ${major}.${minor}.${patch}`,
		);
	}
	if (totalLength !== bytes.byteLength) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Graphics extension length mismatch');
	}
	if (reservedHeader !== 0) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Graphics header is not canonical',
		);
	}
	if (
		imageCount > PROTOCOL_LIMITS.maxTerminalGraphicsImages ||
		placementCount > PROTOCOL_LIMITS.maxTerminalGraphicsPlacements ||
		declaredImageBytes > PROTOCOL_LIMITS.maxTerminalGraphicsResourceBytes
	) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Graphics table exceeds its limit');
	}
	const kind = updateKindFromCode(kindCode);
	if (!kind.ok) return kind;
	if (kind.value === TerminalGraphicsUpdateKind.placements && imageCount !== 0) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Placement update contains image records',
		);
	}

	const images: TerminalGraphicsImage[] = [];
	const imageIds = new Set<number>();
	let actualImageBytes = 0;
	for (let index = 0; index < imageCount; index += 1) {
		const id = reader.readUint32();
		const formatValue = reader.readUint8();
		const reservedFormat = reader.readBytes(3);
		const generation = reader.readUint64();
		const width = reader.readUint32();
		const height = reader.readUint32();
		const dataLength = reader.readUint32();
		const reservedImage = reader.readUint32();
		if (reader.failure) return reader.failure;
		if (reservedImage !== 0 || reservedFormat.some((value) => value !== 0)) {
			return protocolFailure(
				ProtocolErrorCode.unknownRequiredFlag,
				'Graphics image is not canonical',
			);
		}
		const format = formatFromCode(formatValue);
		if (!format.ok) return format;
		const data = new Uint8Array(reader.readBytes(dataLength));
		if (reader.failure) return reader.failure;
		const image: TerminalGraphicsImage = {
			data,
			format: format.value,
			generation,
			height,
			id,
			width,
		};
		const valid = validImageShape(image);
		if (!valid.ok) return valid;
		if (imageIds.has(id)) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics image IDs must be unique');
		}
		imageIds.add(id);
		actualImageBytes += dataLength;
		if (actualImageBytes > PROTOCOL_LIMITS.maxTerminalGraphicsResourceBytes) {
			return protocolFailure(ProtocolErrorCode.limitExceeded, 'Graphics pixels exceed their limit');
		}
		images.push(image);
	}
	if (actualImageBytes !== declaredImageBytes) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Graphics image bytes disagree');
	}
	if (images.length > 0 && resourceRevision === 0n) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics resources require a revision');
	}

	const placements: TerminalGraphicsPlacement[] = [];
	for (let index = 0; index < placementCount; index += 1) {
		const imageId = reader.readUint32();
		const placementId = reader.readUint32();
		const column = decodeInt32(reader.readUint32());
		const row = decodeInt32(reader.readUint32());
		const offsetX = reader.readUint32();
		const offsetY = reader.readUint32();
		const sourceX = reader.readUint32();
		const sourceY = reader.readUint32();
		const sourceWidth = reader.readUint32();
		const sourceHeight = reader.readUint32();
		const destinationWidth = reader.readUint32();
		const destinationHeight = reader.readUint32();
		const z = decodeInt32(reader.readUint32());
		const flags = reader.readUint32();
		if (reader.failure) return reader.failure;
		if ((flags & ~PLACEMENT_FLAG_INTERNAL_ID) !== 0) {
			return protocolFailure(
				ProtocolErrorCode.unknownRequiredFlag,
				'Graphics placement has unknown flags',
			);
		}
		const placement: TerminalGraphicsPlacement = {
			column,
			destinationHeight,
			destinationWidth,
			imageId,
			internalId: (flags & PLACEMENT_FLAG_INTERNAL_ID) !== 0,
			offsetX,
			offsetY,
			placementId,
			row,
			sourceHeight,
			sourceWidth,
			sourceX,
			sourceY,
			z,
		};
		if (!validPlacementShape(placement)) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics placement is invalid');
		}
		if (kind.value === TerminalGraphicsUpdateKind.snapshot && !imageIds.has(imageId)) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Graphics placement references no image',
			);
		}
		const image = images.find((candidate) => candidate.id === imageId);
		if (
			image !== undefined &&
			(sourceX + sourceWidth > image.width || sourceY + sourceHeight > image.height)
		) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Graphics crop exceeds its image');
		}
		placements.push(placement);
	}
	if (reader.offset !== bytes.byteLength) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'Graphics extension has trailing bytes',
		);
	}
	return protocolSuccess({ animationTick, images, kind: kind.value, placements, resourceRevision });
}
