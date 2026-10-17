import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';

/** @internal Exact source inputs consumed by the development Host build. */
export async function terminalHostInputDigest(terminalHostRoot: string): Promise<string> {
	const sourcePaths = await collectTerminalHostInputPaths(path.join(terminalHostRoot, 'src'));
	const inputPaths = [
		path.join(terminalHostRoot, 'build.zig'),
		path.join(terminalHostRoot, 'build.zig.zon'),
		path.resolve(
			terminalHostRoot,
			'../../crates/local-site-runtime/src/windows_powershell_initialization.ps1',
		),
		path.resolve(
			terminalHostRoot,
			'../../crates/local-site-runtime/src/windows_powershell_reader.ps1',
		),
		...sourcePaths,
	].sort();
	const hash = createHash('sha256');
	for (const inputPath of inputPaths) {
		hash.update(path.relative(terminalHostRoot, inputPath).split(path.sep).join('/'));
		hash.update('\0');
		hash.update(await readFile(inputPath));
		hash.update('\0');
	}
	return hash.digest('hex');
}

async function collectTerminalHostInputPaths(directory: string): Promise<string[]> {
	const entries = await readdir(directory, { withFileTypes: true });
	const paths = await Promise.all(
		entries.map(async (entry): Promise<string[]> => {
			const entryPath = path.join(directory, entry.name);
			if (entry.isDirectory()) return collectTerminalHostInputPaths(entryPath);
			return entry.isFile() && ['.c', '.h', '.zig'].includes(path.extname(entry.name))
				? [entryPath]
				: [];
		}),
	);
	return paths.flat();
}
