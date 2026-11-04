type WorkbenchTerminalCurrentDirectoryObservation =
	| { readonly currentCwd: string | null; readonly kind: 'reported' }
	| { readonly kind: 'ignored' };

export interface WorkbenchTerminalMetadataObservation {
	readonly currentDirectory: WorkbenchTerminalCurrentDirectoryObservation;
	readonly naturalTitle: string | null;
}

interface RawTerminalMetadataObservation {
	readonly currentDirectory: string | null;
	readonly title: string | null;
}

/** A `file:` URL's path to a Windows directory, which Windows itself only accepts without the slash. */
const WINDOWS_DRIVE_URL_PATH = /^\/[A-Za-z]:[/\\]/u;

export function resolveTerminalMetadataObservation(
	metadata: RawTerminalMetadataObservation,
	terminalHostName: string,
): WorkbenchTerminalMetadataObservation {
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
	const onThisHost = asciiEqualsIgnoreCase(host, terminalHostName);
	let path: string;
	try {
		path = decodeURIComponent(remainder.slice(pathStart));
	} catch {
		return { kind: 'ignored' };
	}
	if (WINDOWS_DRIVE_URL_PATH.test(path)) {
		// Windows shells often leave the host out, and the drive still says which machine this is.
		return onThisHost || host.length === 0 || asciiEqualsIgnoreCase(host, 'localhost')
			? { currentCwd: path.slice(1).replaceAll('/', '\\'), kind: 'reported' }
			: { kind: 'ignored' };
	}
	return onThisHost && host.length > 0 && !asciiEqualsIgnoreCase(host, 'localhost')
		? { currentCwd: path, kind: 'reported' }
		: { kind: 'ignored' };
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
