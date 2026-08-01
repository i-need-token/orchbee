import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

import {
	encodeFocusInput,
	encodeKeyInput,
	encodeMouseInput,
	encodePasteInput,
	encodeProtocolEnvelope,
	encodeTerminalFrame,
	encodeTextInput,
	encodeWriteInput,
} from '../dist/index.js';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fixturesDirectory = path.join(packageRoot, 'fixtures');
const canonicalPath = path.join(fixturesDirectory, 'canonical-v1.json');
const framePath = path.join(fixturesDirectory, 'terminal-frame-v2.bin');
const envelopePath = path.join(fixturesDirectory, 'transport-envelope-v1.bin');
const inputPath = path.join(fixturesDirectory, 'write-input-v1.bin');
const keyInputPath = path.join(fixturesDirectory, 'key-input-v1_1.bin');
const textInputPath = path.join(fixturesDirectory, 'text-input-v1_1.bin');
const mouseInputPath = path.join(fixturesDirectory, 'mouse-input-v1_1.bin');
const focusInputPath = path.join(fixturesDirectory, 'focus-input-v1_1.bin');
const pasteInputPath = path.join(fixturesDirectory, 'paste-input-v1_1.bin');
const malformedPath = path.join(fixturesDirectory, 'malformed-frame-v2.json');
const checkOnly = process.argv.includes('--check');

function bytesFromBase64(value) {
	return new Uint8Array(Buffer.from(value, 'base64'));
}

function frameFromJson(value) {
	return {
		...value,
		baseRevision: value.baseRevision === null ? null : BigInt(value.baseRevision),
		extensions: value.extensions.map((extension) => ({
			data: bytesFromBase64(extension.dataBase64),
			kind: extension.kind,
			required: extension.required,
		})),
		generation: BigInt(value.generation),
		revision: BigInt(value.revision),
		scrollbackRows: BigInt(value.scrollbackRows),
		sequence: BigInt(value.sequence),
	};
}

function encodeOrThrow(result, name) {
	if (!result.ok)
		throw new Error(`${name} fixture failed to encode: ${result.code}: ${result.detail}`);
	return result.value;
}

function locateTables(bytes) {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const stringCount = view.getUint32(124, true);
	const styleCount = view.getUint16(128, true);
	let offset = 160;
	for (let index = 0; index < stringCount; index += 1) {
		const length = view.getUint32(offset, true);
		offset += 4 + length;
	}
	return { firstRow: offset + styleCount * 20, firstStyle: offset };
}

function malformedCorpus(frameBytes) {
	const invalidMagic = frameBytes.slice();
	invalidMagic[0] = 0;

	const lengthMismatch = frameBytes.slice();
	new DataView(lengthMismatch.buffer).setUint32(8, lengthMismatch.byteLength - 1, true);

	const invalidUtf8 = frameBytes.slice();
	invalidUtf8[164] = 0xff;

	const rowOutOfBounds = frameBytes.slice();
	new DataView(rowOutOfBounds.buffer).setUint16(locateTables(rowOutOfBounds).firstRow, 2, true);

	const unknownStyleFlag = frameBytes.slice();
	new DataView(unknownStyleFlag.buffer).setUint16(
		locateTables(unknownStyleFlag).firstStyle + 16,
		0x0100,
		true,
	);

	const unknownRequiredExtension = frameBytes.slice();
	new DataView(unknownRequiredExtension.buffer).setUint16(
		unknownRequiredExtension.byteLength - 10,
		0x8000,
		true,
	);

	return [
		{
			bytesBase64: Buffer.from(frameBytes.subarray(0, 120)).toString('base64'),
			code: 'truncated',
			name: 'truncated-header',
		},
		{
			bytesBase64: Buffer.from(invalidMagic).toString('base64'),
			code: 'invalid-magic',
			name: 'invalid-magic',
		},
		{
			bytesBase64: Buffer.from(lengthMismatch).toString('base64'),
			code: 'length-mismatch',
			name: 'length-mismatch',
		},
		{
			bytesBase64: Buffer.from(invalidUtf8).toString('base64'),
			code: 'invalid-utf8',
			name: 'invalid-utf8',
		},
		{
			bytesBase64: Buffer.from(rowOutOfBounds).toString('base64'),
			code: 'invalid-field',
			name: 'row-out-of-bounds',
		},
		{
			bytesBase64: Buffer.from(unknownStyleFlag).toString('base64'),
			code: 'unknown-required-flag',
			name: 'unknown-style-flag',
		},
		{
			bytesBase64: Buffer.from(unknownRequiredExtension).toString('base64'),
			code: 'unknown-required-flag',
			name: 'unknown-required-extension',
		},
	];
}

async function compareFile(filePath, expected) {
	let actual;
	try {
		actual = await readFile(filePath);
	} catch (error) {
		if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
			throw new Error(
				`${path.relative(packageRoot, filePath)} is missing; run pnpm fixtures:generate`,
				{ cause: error },
			);
		}
		throw error;
	}
	if (!actual.equals(Buffer.from(expected))) {
		throw new Error(`${path.relative(packageRoot, filePath)} is stale; run pnpm fixtures:generate`);
	}
}

const canonical = JSON.parse(await readFile(canonicalPath, 'utf8'));
const frameBytes = encodeOrThrow(
	encodeTerminalFrame(frameFromJson(canonical.frame)),
	'terminal frame',
);
const envelopeBytes = encodeOrThrow(
	encodeProtocolEnvelope({ ...canonical.transportEnvelope, payload: frameBytes }),
	'transport envelope',
);
const inputBytes = encodeOrThrow(
	encodeWriteInput({
		attachmentId: canonical.writeInput.attachmentId,
		bytes: bytesFromBase64(canonical.writeInput.bytesBase64),
	}),
	'write input',
);
const keyInputBytes = encodeOrThrow(encodeKeyInput(canonical.keyInput), 'key input');
const textInputBytes = encodeOrThrow(encodeTextInput(canonical.textInput), 'text input');
const mouseInputBytes = encodeOrThrow(encodeMouseInput(canonical.mouseInput), 'mouse input');
const focusInputBytes = encodeOrThrow(encodeFocusInput(canonical.focusInput), 'focus input');
const pasteInputBytes = encodeOrThrow(encodePasteInput(canonical.pasteInput), 'paste input');
const malformedBytes = new TextEncoder().encode(
	`${JSON.stringify({ cases: malformedCorpus(frameBytes) }, null, '\t')}\n`,
);

if (checkOnly) {
	await compareFile(framePath, frameBytes);
	await compareFile(envelopePath, envelopeBytes);
	await compareFile(inputPath, inputBytes);
	await compareFile(keyInputPath, keyInputBytes);
	await compareFile(textInputPath, textInputBytes);
	await compareFile(mouseInputPath, mouseInputBytes);
	await compareFile(focusInputPath, focusInputBytes);
	await compareFile(pasteInputPath, pasteInputBytes);
	await compareFile(malformedPath, malformedBytes);
} else {
	await writeFile(framePath, frameBytes);
	await writeFile(envelopePath, envelopeBytes);
	await writeFile(inputPath, inputBytes);
	await writeFile(keyInputPath, keyInputBytes);
	await writeFile(textInputPath, textInputBytes);
	await writeFile(mouseInputPath, mouseInputBytes);
	await writeFile(focusInputPath, focusInputBytes);
	await writeFile(pasteInputPath, pasteInputBytes);
	await writeFile(malformedPath, malformedBytes);
}
