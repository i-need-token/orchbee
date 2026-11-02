export const MAXIMUM_TERMINAL_FILE_DROP_FILES = 32;
export const MAXIMUM_TERMINAL_FILE_DROP_INPUT_CHARACTERS = 4096;

const TERMINAL_FILE_DROP_ESCAPE_CHARACTERS = new Set(`\\ ()[]{}<>"'\`!#$&;|*?`);

export type TerminalFileDropRejectionReason =
	| 'empty'
	| 'input-too-large'
	| 'invalid-path'
	| 'too-many-files'
	| 'unavailable-path'
	| 'unsupported';

type TerminalFileDropPreparation =
	| {
			readonly kind: 'ready';
			readonly paths: readonly string[];
			readonly text: string;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: TerminalFileDropRejectionReason;
	  };

interface TerminalFileDropPathResolver {
	readonly platform: unknown;
	getPathForFile(file: File): unknown | Promise<unknown>;
}

export interface TerminalFileDropCapability {
	prepare(files: readonly File[]): Promise<TerminalFileDropPreparation>;
}

type TerminalFileDropDataTransfer = Pick<DataTransfer, 'files' | 'items' | 'types'>;

export function containsTerminalFileDrop(transfer: TerminalFileDropDataTransfer): boolean {
	return (
		transfer.files.length > 0 ||
		Array.from(transfer.items).some((item) => item.kind === 'file') ||
		Array.from(transfer.types).includes('Files')
	);
}

export function extractTerminalFileDropFiles(
	transfer: TerminalFileDropDataTransfer,
): readonly File[] {
	const files = Array.from(transfer.files);
	if (files.length > 0) return files;

	const itemFiles: File[] = [];
	try {
		for (const item of Array.from(transfer.items)) {
			if (item.kind !== 'file') continue;
			const file = item.getAsFile();
			if (file !== null) itemFiles.push(file);
		}
	} catch {
		return [];
	}
	return itemFiles;
}

/** @internal Kept without a provider until the migration decides file drop; its unit test covers path escaping. */
export function createTerminalFileDropCapability(
	resolver: TerminalFileDropPathResolver,
): TerminalFileDropCapability {
	return {
		prepare: async (files) => await prepareTerminalFileDrop(files, resolver),
	};
}

async function prepareTerminalFileDrop(
	files: readonly File[],
	resolver: TerminalFileDropPathResolver,
): Promise<TerminalFileDropPreparation> {
	if (resolver.platform !== 'darwin' && resolver.platform !== 'linux') {
		return { kind: 'rejected', reason: 'unsupported' };
	}
	if (files.length === 0) return { kind: 'rejected', reason: 'empty' };
	if (files.length > MAXIMUM_TERMINAL_FILE_DROP_FILES) {
		return { kind: 'rejected', reason: 'too-many-files' };
	}

	let resolved: readonly unknown[];
	try {
		resolved = await Promise.all(files.map(async (file) => await resolver.getPathForFile(file)));
	} catch {
		return { kind: 'rejected', reason: 'unavailable-path' };
	}

	const paths: string[] = [];
	const escapedPaths: string[] = [];
	let inputCharacters = 0;
	for (const candidate of resolved) {
		if (typeof candidate !== 'string' || candidate.length === 0) {
			return { kind: 'rejected', reason: 'unavailable-path' };
		}
		if (!candidate.startsWith('/')) {
			return { kind: 'rejected', reason: 'invalid-path' };
		}
		if (escapedPaths.length > 0) inputCharacters += 1;
		let escapedPath = '';
		for (const character of candidate) {
			const codepoint = character.codePointAt(0) ?? 0;
			if (isTerminalFileDropControl(codepoint)) {
				return { kind: 'rejected', reason: 'invalid-path' };
			}
			const escapedCharacter = TERMINAL_FILE_DROP_ESCAPE_CHARACTERS.has(character)
				? `\\${character}`
				: character;
			escapedPath += escapedCharacter;
			inputCharacters += TERMINAL_FILE_DROP_ESCAPE_CHARACTERS.has(character) ? 2 : 1;
			if (inputCharacters > MAXIMUM_TERMINAL_FILE_DROP_INPUT_CHARACTERS) {
				return { kind: 'rejected', reason: 'input-too-large' };
			}
		}
		paths.push(candidate);
		escapedPaths.push(escapedPath);
	}

	return {
		kind: 'ready',
		paths,
		text: escapedPaths.join(' '),
	};
}

function isTerminalFileDropControl(codepoint: number): boolean {
	return codepoint <= 0x1f || (codepoint >= 0x7f && codepoint <= 0x9f);
}
