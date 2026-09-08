import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

if (process.platform !== 'win32') {
	throw new Error('The semantic endpoint Windows security verifier requires Windows.');
}
const executeFile = promisify(execFile);
const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sourceRoot = path.join(desktopRoot, 'src', 'native', 'semantic-command-endpoint');
const root = await mkdtemp(path.join(tmpdir(), 'ob-sc-security-'));
try {
	const { stdout: version } = await executeFile('zig', ['version'], { encoding: 'utf8' });
	if (version.trim() !== '0.16.0')
		throw new Error('Windows security verification requires Zig 0.16.0.');
	const executable = path.join(root, 'security.exe');
	await executeFile(
		'zig',
		[
			'cc',
			'-std=c17',
			'-O2',
			'-Wall',
			'-Wextra',
			'-Wpedantic',
			'-Werror',
			'-municode',
			path.join(sourceRoot, 'tests', 'windows_security.c'),
			'-ladvapi32',
			'-o',
			executable,
		],
		{ encoding: 'utf8' },
	);
	const target = path.join(root, 'target');
	const junction = path.join(root, 'junction');
	await mkdir(target);
	await symlink(target, junction, 'junction');
	const { stdout, stderr } = await executeFile(
		executable,
		[path.join(root, 'runtime'), junction, target],
		{
			encoding: 'utf8',
		},
	);
	process.stdout.write(stdout);
	process.stderr.write(stderr);
	const acceptExecutable = path.join(root, 'accept.exe');
	await executeFile(
		'zig',
		[
			'cc',
			'-std=c17',
			'-O2',
			'-Wall',
			'-Wextra',
			'-Wpedantic',
			'-Werror',
			'-municode',
			path.join(sourceRoot, 'tests', 'windows_accept.c'),
			'-ladvapi32',
			'-o',
			acceptExecutable,
		],
		{ encoding: 'utf8' },
	);
	const acceptResult = await executeFile(acceptExecutable, [path.join(root, 'accept-runtime')], {
		encoding: 'utf8',
		timeout: 20_000,
	});
	process.stdout.write(acceptResult.stdout);
	process.stderr.write(acceptResult.stderr);
	const writeExecutable = path.join(root, 'write.exe');
	await executeFile(
		'zig',
		[
			'cc',
			'-std=c17',
			'-O2',
			'-Wall',
			'-Wextra',
			'-Wpedantic',
			'-Werror',
			path.join(sourceRoot, 'tests', 'windows_write.c'),
			'-ladvapi32',
			'-o',
			writeExecutable,
		],
		{ encoding: 'utf8' },
	);
	const writeResult = await executeFile(writeExecutable, [path.join(root, 'write-runtime')], {
		encoding: 'utf8',
		timeout: 20_000,
	});
	process.stdout.write(writeResult.stdout);
	process.stderr.write(writeResult.stderr);
	const persistentExecutable = path.join(root, 'persistent-accept.exe');
	await executeFile(
		'zig',
		[
			'cc',
			'-std=c17',
			'-O2',
			'-Wall',
			'-Wextra',
			'-Wpedantic',
			'-Werror',
			path.join(sourceRoot, 'tests', 'windows_persistent_accept.c'),
			'-ladvapi32',
			'-o',
			persistentExecutable,
		],
		{ encoding: 'utf8' },
	);
	const persistentResult = await executeFile(
		persistentExecutable,
		[path.join(root, 'persistent-runtime')],
		{ encoding: 'utf8', timeout: 20_000 },
	);
	process.stdout.write(persistentResult.stdout);
	process.stderr.write(persistentResult.stderr);
} finally {
	await rm(root, { recursive: true, force: true });
}
