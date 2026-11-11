import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
	copyFileSync,
	existsSync,
	lstatSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	realpathSync,
	renameSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(readFileSync(path.join(packageRoot, 'windows-conpty.json'), 'utf8'));
const maximumArchiveBytes = 32 * 1024 * 1024;
const maximumBinaryBytes = 64 * 1024 * 1024;

function removeOwnedTemporary(directory, parent) {
	const actual = realpathSync(directory);
	if (path.dirname(actual) !== parent || !path.basename(actual).startsWith('orchbee-conpty-')) {
		throw new Error('Refusing cleanup outside owned ConPTY staging directory');
	}
	rmSync(actual, { recursive: true });
}

export function verifyConptyBundle(directory) {
	const root = lstatSync(directory);
	if (!root.isDirectory() || root.isSymbolicLink())
		throw new Error('Unsafe ConPTY bundle directory');
	for (const entry of [...Object.values(manifest.files), manifest.license]) {
		const target = path.join(directory, entry.name);
		const metadata = lstatSync(target);
		if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > maximumBinaryBytes) {
			throw new Error('Unsafe ConPTY bundle file');
		}
		const digest = createHash('sha256').update(readFileSync(target)).digest('hex');
		if (digest !== entry.sha256) throw new Error(`ConPTY integrity mismatch: ${entry.name}`);
	}
}

export async function stageConptyBundle(output) {
	if (process.platform !== 'win32' || process.arch !== 'x64')
		throw new Error('ConPTY staging requires native Windows x64');
	if (!path.isAbsolute(output) || path.normalize(output) !== output || existsSync(output)) {
		throw new Error('ConPTY output must be a new normalized absolute directory');
	}
	const parent = realpathSync(path.dirname(output));
	const temporary = mkdtempSync(path.join(parent, 'orchbee-conpty-'));
	try {
		const response = await fetch(manifest.url, { signal: AbortSignal.timeout(30_000) });
		if (!response.ok || response.body === null) throw new Error('ConPTY package download failed');
		const chunks = [];
		let size = 0;
		for await (const chunk of response.body) {
			size += chunk.byteLength;
			if (size > maximumArchiveBytes) throw new Error('ConPTY package exceeds its bound');
			chunks.push(chunk);
		}
		const bytes = Buffer.concat(chunks, size);
		if (createHash('sha256').update(bytes).digest('hex') !== manifest.sha256) {
			throw new Error('ConPTY package digest mismatch');
		}
		const archive = path.join(temporary, 'package.zip');
		const expanded = path.join(temporary, 'expanded');
		writeFileSync(archive, bytes, { flag: 'wx' });
		const systemRoot = process.env.SystemRoot;
		if (typeof systemRoot !== 'string' || !path.isAbsolute(systemRoot))
			throw new Error('Missing Windows system root');
		// Windows PowerShell must load its own Archive module. A PowerShell 7 parent leaves its module
		// path in the environment, and that module's extraction call does not exist in this host.
		const windowsPowerShell = path.join(systemRoot, 'System32', 'WindowsPowerShell', 'v1.0');
		const inherited = Object.fromEntries(
			Object.entries(process.env).filter(([name]) => name.toLowerCase() !== 'psmodulepath'),
		);
		execFileSync(
			path.join(windowsPowerShell, 'powershell.exe'),
			[
				'-NoLogo',
				'-NoProfile',
				'-NonInteractive',
				'-Command',
				"$ErrorActionPreference='Stop'; Expand-Archive -LiteralPath $env:ORCHBEE_CONPTY_ARCHIVE -DestinationPath $env:ORCHBEE_CONPTY_EXPANDED",
			],
			{
				env: {
					...inherited,
					PSModulePath: path.join(windowsPowerShell, 'Modules'),
					ORCHBEE_CONPTY_ARCHIVE: archive,
					ORCHBEE_CONPTY_EXPANDED: expanded,
				},
				stdio: 'inherit',
				timeout: 30_000,
				windowsHide: true,
			},
		);
		const prepared = path.join(temporary, 'prepared');
		mkdirSync(prepared);
		for (const entry of Object.values(manifest.files)) {
			copyFileSync(path.join(expanded, entry.source), path.join(prepared, entry.name));
		}
		copyFileSync(
			path.join(packageRoot, 'licenses', manifest.license.name),
			path.join(prepared, manifest.license.name),
		);
		verifyConptyBundle(prepared);
		renameSync(prepared, output);
	} finally {
		removeOwnedTemporary(temporary, parent);
	}
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	if (process.argv.length !== 4 || process.argv[2] !== '--output')
		throw new Error('Usage: windows-conpty.mjs --output ABSOLUTE_NEW_DIRECTORY');
	await stageConptyBundle(process.argv[3]);
}
