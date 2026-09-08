import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
	constants,
	copyFileSync,
	existsSync,
	mkdtempSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const artifacts = ['orchbee', 'orchbee-server', 'orchbee-worker', 'orchbee-terminal-host'];

function options(args) {
	const result = {};
	for (let i = 0; i < args.length; i += 2) {
		if (!['--wsl', '--output', '--cargo-target-dir'].includes(args[i]) || !args[i + 1]) {
			throw new Error(
				'Usage: node scripts/stage-native-site.mjs --output ABSOLUTE_NEW_DIRECTORY [--wsl DISTRO] [--cargo-target-dir ABSOLUTE_DIRECTORY]',
			);
		}
		if (result[args[i]] !== undefined) throw new Error('Duplicate staging option');
		result[args[i]] = args[i + 1];
	}
	if (!result['--output'] || !path.posix.isAbsolute(result['--output']))
		throw new Error('An absolute Linux output directory is required');
	if (process.platform === 'win32' && !result['--wsl'])
		throw new Error('Windows staging requires --wsl DISTRO');
	if (process.platform !== 'linux' && process.platform !== 'win32')
		throw new Error('Local startup staging currently supports Linux and Windows/WSL');
	return result;
}
function main() {
	const selected = options(process.argv.slice(2));
	const distro = selected['--wsl'];
	const linux = (command, args, capture = false) => {
		const executable = distro ? 'wsl.exe' : command;
		const argv = distro
			? ['-d', distro, '--exec', 'bash', '-lc', 'exec "$@"', 'native-stage', command, ...args]
			: args;
		return execFileSync(executable, argv, {
			cwd: root,
			encoding: 'utf8',
			stdio: capture ? 'pipe' : 'inherit',
		});
	};
	const accessible = (linuxPath) =>
		distro ? linux('wslpath', ['-w', linuxPath], true).trim() : linuxPath;
	const output = selected['--output'];
	const normalized = path.posix.normalize(output);
	if (normalized !== output || output === '/')
		throw new Error('Output must be a normalized new directory');
	if (existsSync(accessible(output))) throw new Error('Refusing to overwrite an existing cohort');
	const checkout = createHash('sha256').update(root).digest('hex').slice(0, 12);
	const target =
		selected['--cargo-target-dir'] ??
		(distro ? '/tmp/orchbee-native-' + checkout + '-target' : path.join(root, 'target'));
	if (!path.posix.isAbsolute(target)) throw new Error('Cargo target directory must be absolute');
	linux('cargo', [
		'build',
		'--release',
		'--locked',
		'--target-dir',
		target,
		'-p',
		'orchbee',
		'-p',
		'orchbee-server',
		'-p',
		'orchbee-worker',
	]);
	const temporary = mkdtempSync(path.join(tmpdir(), 'orchbee-native-host-'));
	const hostPrefix = path.join(temporary, 'host');
	const zigArguments = ['build', '-Doptimize=ReleaseSafe', '--prefix', hostPrefix];
	if (distro) {
		if (linux('uname', ['-m'], true).trim() !== 'x86_64')
			throw new Error('Windows cross-staging currently requires x86_64 Linux');
		zigArguments.push('-Dtarget=x86_64-linux-musl');
	}
	execFileSync('zig', zigArguments, {
		cwd: path.join(root, 'apps', 'terminal-host'),
		stdio: 'inherit',
	});
	linux('mkdir', ['-m', '700', '--', output]);
	const destination = accessible(output);
	const hashes = {};
	for (const artifact of artifacts) {
		const source =
			artifact === 'orchbee-terminal-host'
				? path.join(hostPrefix, 'bin', artifact)
				: accessible(path.posix.join(target, 'release', artifact));
		const staged = path.join(destination, artifact);
		copyFileSync(source, staged, constants.COPYFILE_EXCL);
		linux('chmod', ['700', '--', path.posix.join(output, artifact)]);
		hashes[artifact] = createHash('sha256').update(readFileSync(staged)).digest('hex');
	}
	writeFileSync(
		path.join(destination, 'cohort.json'),
		JSON.stringify({ schemaVersion: 1, artifacts: hashes }) + '\n',
		{ flag: 'wx', mode: 0o600 },
	);
	linux('chmod', ['600', '--', path.posix.join(output, 'cohort.json')]);
	process.stdout.write(
		'Staged Linux development cohort: ' +
			output +
			'\nRun in an interactive Linux terminal: ' +
			path.posix.join(output, 'orchbee') +
			'\n',
	);
}
main();
