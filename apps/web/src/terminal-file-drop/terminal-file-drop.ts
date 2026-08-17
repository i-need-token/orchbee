export const MAXIMUM_TERMINAL_FILE_DROP_FILES = 32;
export const MAXIMUM_TERMINAL_FILE_DROP_PREVIEW_CHARACTERS = 4096;

export type TerminalFileDropRejectionReason =
	| 'empty'
	| 'invalid-path'
	| 'preview-too-large'
	| 'too-many-files'
	| 'unavailable-path'
	| 'unsupported';

type TerminalFileDropPreparation =
	| {
			readonly containsNewline: boolean;
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
	const quotedPaths: string[] = [];
	let containsNewline = false;
	let previewCharacters = 0;
	for (const candidate of resolved) {
		if (typeof candidate !== 'string' || candidate.length === 0) {
			return { kind: 'rejected', reason: 'unavailable-path' };
		}
		if (!candidate.startsWith('/')) {
			return { kind: 'rejected', reason: 'invalid-path' };
		}
		if (quotedPaths.length > 0) previewCharacters += 1;
		previewCharacters += 2;
		for (const character of candidate) {
			const codepoint = character.codePointAt(0) ?? 0;
			if (isPasteUnsafeControl(codepoint)) {
				return { kind: 'rejected', reason: 'invalid-path' };
			}
			if (character === '\n') containsNewline = true;
			previewCharacters += character === "'" ? 4 : 1;
			if (previewCharacters > MAXIMUM_TERMINAL_FILE_DROP_PREVIEW_CHARACTERS) {
				return { kind: 'rejected', reason: 'preview-too-large' };
			}
		}
		paths.push(candidate);
		quotedPaths.push(posixShellWord(candidate));
	}

	return {
		containsNewline,
		kind: 'ready',
		paths,
		text: quotedPaths.join(' '),
	};
}

function isPasteUnsafeControl(codepoint: number): boolean {
	return (
		codepoint <= 0x08 ||
		codepoint === 0x0b ||
		codepoint === 0x0c ||
		(codepoint >= 0x0e && codepoint <= 0x1f) ||
		(codepoint >= 0x7f && codepoint <= 0x9f)
	);
}

function posixShellWord(path: string): string {
	return `'${path.replaceAll("'", "'\\''")}'`;
}
