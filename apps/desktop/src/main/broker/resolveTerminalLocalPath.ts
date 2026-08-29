import { homedir } from 'node:os';
import path from 'node:path';

const utf8Encoder = new TextEncoder();

export function resolveTerminalLocalPath(target: string, homeDirectory = homedir()): string {
	if (
		target.length === 0 ||
		containsControlCharacter(target) ||
		utf8Encoder.encode(target).byteLength > 64 * 1024
	) {
		throw new Error('The terminal file target is invalid.');
	}
	let expanded = target;
	if (target === '~') expanded = homeDirectory;
	else if (target.startsWith('~/')) expanded = path.join(homeDirectory, target.slice(2));
	if (!path.isAbsolute(expanded)) {
		throw new Error('The terminal file target must be absolute.');
	}
	return path.normalize(expanded);
}

function containsControlCharacter(value: string): boolean {
	for (const character of value) {
		const codePoint = character.codePointAt(0);
		if (
			codePoint !== undefined &&
			(codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f))
		) {
			return true;
		}
	}
	return false;
}
