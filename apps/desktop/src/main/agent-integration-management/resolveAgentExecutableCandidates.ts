import path from 'node:path';

const EXECUTABLE_NAME_PATTERN = /^[a-z][a-z0-9-]{0,31}$/u;
const MAXIMUM_SEARCH_DIRECTORIES_PER_SOURCE = 256;
const MAXIMUM_SEARCH_DIRECTORY_CHARACTERS = 4096;

export type AgentExecutableCandidateResolution =
	| { readonly kind: 'candidates'; readonly paths: readonly string[] }
	| { readonly kind: 'invalid-name' };

export function resolveAgentExecutableCandidates(
	executableName: string,
	searchPaths: readonly (string | undefined)[],
): AgentExecutableCandidateResolution {
	if (!EXECUTABLE_NAME_PATTERN.test(executableName)) return { kind: 'invalid-name' };
	const candidates: string[] = [];
	const seenCandidates = new Set<string>();
	for (const searchPath of searchPaths) {
		if (searchPath === undefined) continue;
		const directories = searchPath.split(path.delimiter, MAXIMUM_SEARCH_DIRECTORIES_PER_SOURCE);
		for (const directory of directories) {
			if (
				directory.length === 0 ||
				directory.length > MAXIMUM_SEARCH_DIRECTORY_CHARACTERS ||
				hasControlCharacter(directory) ||
				!path.isAbsolute(directory)
			) {
				continue;
			}
			const candidate = path.join(directory, executableName);
			if (seenCandidates.has(candidate)) continue;
			seenCandidates.add(candidate);
			candidates.push(candidate);
		}
	}
	return { kind: 'candidates', paths: candidates };
}

function hasControlCharacter(value: string): boolean {
	for (const character of value) {
		const characterCode = character.charCodeAt(0);
		if (characterCode <= 0x1f || characterCode === 0x7f) return true;
	}
	return false;
}
