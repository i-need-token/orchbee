import type { TerminalPresentationCloseActivity } from '../../terminal-presentation';

export type WorkbenchTerminalCurrentDirectoryObservation =
	| { readonly currentCwd: string | null; readonly kind: 'reported' }
	| { readonly kind: 'ignored' };

export interface WorkbenchTerminalMetadataObservation {
	readonly closeActivity: TerminalPresentationCloseActivity;
	readonly currentDirectory: WorkbenchTerminalCurrentDirectoryObservation;
	readonly naturalTitle: string | null;
}

export interface RawTerminalMetadataObservation {
	readonly closeActivity: TerminalPresentationCloseActivity;
	readonly currentDirectory: string | null;
	readonly title: string | null;
}

export type RawTerminalNaturalMetadataObservation = Omit<
	RawTerminalMetadataObservation,
	'closeActivity'
>;

export function resolveTerminalMetadataObservation(
	metadata: RawTerminalMetadataObservation,
	terminalHostName: string,
): WorkbenchTerminalMetadataObservation {
	return {
		closeActivity: metadata.closeActivity,
		...resolveTerminalNaturalMetadataObservation(metadata, terminalHostName),
	};
}

export function resolveTerminalNaturalMetadataObservation(
	metadata: RawTerminalNaturalMetadataObservation,
	terminalHostName: string,
): Omit<WorkbenchTerminalMetadataObservation, 'closeActivity'> {
	return {
		currentDirectory:
			metadata.currentDirectory === null
				? { currentCwd: null, kind: 'reported' }
				: resolveOsc7CurrentDirectory(metadata.currentDirectory, terminalHostName),
		naturalTitle: metadata.title?.trim().replaceAll('\0', '') ?? null,
	};
}

function resolveOsc7CurrentDirectory(
	value: string,
	terminalHostName: string,
): WorkbenchTerminalCurrentDirectoryObservation {
	if (!value.startsWith('file://')) return { kind: 'ignored' };
	const remainder = value.slice('file://'.length);
	const pathStart = remainder.indexOf('/');
	if (pathStart === -1) return { kind: 'ignored' };
	const host = remainder.slice(0, pathStart);
	if (
		host.length === 0 ||
		asciiEqualsIgnoreCase(host, 'localhost') ||
		!asciiEqualsIgnoreCase(host, terminalHostName)
	) {
		return { kind: 'ignored' };
	}
	try {
		return {
			currentCwd: decodeURIComponent(remainder.slice(pathStart)),
			kind: 'reported',
		};
	} catch {
		return { kind: 'ignored' };
	}
}

function asciiEqualsIgnoreCase(left: string, right: string): boolean {
	if (left.length !== right.length) return false;
	for (let index = 0; index < left.length; index += 1) {
		if (asciiLowerCode(left.charCodeAt(index)) !== asciiLowerCode(right.charCodeAt(index))) {
			return false;
		}
	}
	return true;
}

function asciiLowerCode(code: number): number {
	return code >= 0x41 && code <= 0x5a ? code + 0x20 : code;
}
