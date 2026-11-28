import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
	constants,
	copyFileSync,
	cpSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	realpathSync,
	renameSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { writeProgramNotices } from './third-party-notices.mjs';

// The environment stem names the staged cohort: the shipped `orchbee` (release) or `ob-dev-` / `ob-test-`
// plus a 16-digit lowercase hex digest (ORC-382). Mirrors the grammar in desktop-distribution-profile.ts.
export const ARTIFACT_STEM_PATTERN = /^(?:orchbee|ob-(?:dev|test)-[0-9a-f]{16})$/u;

/**
 * The Terminal Host build options that give a Host its cohort's identity: the environment stem, which
 * the Worker handshake compares, the product version of `apps/orchbee`, and the source commit that
 * release packaging names in `ORCHBEE_SOURCE_COMMIT`, which the Cargo builds read at compile time.
 */
export function terminalHostBuildOptions(root, artifactStem, environment = process.env) {
	const manifest = readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
	const workspacePackage =
		/^\[workspace\.package\]\r?\n([\s\S]*?)(?=^\[|(?![\s\S]))/mu.exec(manifest)?.[1] ?? '';
	const version = /^version = "([^"]+)"$/mu.exec(workspacePackage)?.[1];
	if (version === undefined) throw new Error('Cargo.toml declares no workspace package version');
	const commit = environment.ORCHBEE_SOURCE_COMMIT;
	return [
		`-Denvironment=${artifactStem}`,
		`-Dhost-version=${version}`,
		...(commit === undefined || commit === '' ? [] : [`-Dsource-commit=${commit}`]),
	];
}

// Only the newly created, empty destination is secured. Existing installations
// are never repaired or overwritten. No user path is interpolated into code.
function privateDirectory(directory) {
	mkdirSync(directory);
	execFileSync(
		'powershell.exe',
		[
			'-NoLogo',
			'-NoProfile',
			'-NonInteractive',
			'-Command',
			"$ErrorActionPreference = 'Stop'; " +
				'$acl = [System.Security.AccessControl.DirectorySecurity]::new(); ' +
				'$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value; ' +
				'$acl.SetSecurityDescriptorSddlForm("D:P(A;OICI;FA;;;" + $sid + ")(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)", [System.Security.AccessControl.AccessControlSections]::Access); ' +
				'[System.IO.Directory]::SetAccessControl($env:ORCHBEE_STAGE_DESTINATION, $acl);',
		],
		{
			env: { ...process.env, ORCHBEE_STAGE_DESTINATION: directory },
			stdio: 'inherit',
			windowsHide: true,
		},
	);
}

function removeOwnedTemporary(directory, parent) {
	const resolved = realpathSync(directory);
	if (
		path.dirname(resolved) !== parent ||
		!path.basename(resolved).startsWith('orchbee-native-host-')
	)
		throw new Error('Refusing cleanup outside the owned Host staging directory');
	rmSync(resolved, { recursive: true });
}

// The probe uses fixed filenames, so the caller must pass its empty, invocation-owned directory.
export function assertWindowsFileSymlinkSupport(directory, createSymlink = symlinkSync) {
	const target = path.join(directory, 'file-symlink-target');
	const link = path.join(directory, 'file-symlink-probe');
	try {
		writeFileSync(target, 'Windows file symlink preflight\n', { flag: 'wx' });
		try {
			createSymlink(path.basename(target), link, 'file');
		} catch (error) {
			if (error?.code === 'EPERM' || error?.code === 'EACCES') {
				throw new Error(
					'Windows native staging requires permission to create file symbolic links. ' +
						'Enable Windows Developer Mode or run this command from an elevated terminal, then retry. ' +
						'See apps/terminal-host/README.md#development.',
					{ cause: error },
				);
			}
			throw error;
		}
	} finally {
		rmSync(link, { force: true });
		rmSync(target, { force: true });
	}
}

export function stageWindows(root, output, cargoTarget, verifyCapsule, artifactStem = 'orchbee') {
	if (!ARTIFACT_STEM_PATTERN.test(artifactStem))
		throw new Error('Artifact stem must be orchbee, ob-dev-<16 hex>, or ob-test-<16 hex>');
	if (
		!/^[A-Za-z]:\\/u.test(output) ||
		path.normalize(output) !== output ||
		path.parse(output).root === output
	)
		throw new Error('Output must be a normalized new absolute directory');
	if (existsSync(output)) throw new Error('Refusing to overwrite an existing cohort');
	const target = cargoTarget ?? path.join(root, 'target');
	if (!path.isAbsolute(target)) throw new Error('Cargo target directory must be absolute');
	const temporaryParent = realpathSync(path.dirname(output));
	const temporary = mkdtempSync(path.join(temporaryParent, 'orchbee-native-host-'));
	try {
		assertWindowsFileSymlinkSupport(temporary);
		execFileSync(
			'cargo',
			[
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
				'-p',
				'orchbee-launcher',
				'-p',
				'orchbee-desktop-launcher',
				...(artifactStem === 'orchbee' ? [] : ['-p', 'orchbee-files']),
			],
			{
				cwd: root,
				env: { ...process.env, ORCHBEE_ARTIFACT_STEM: artifactStem },
				stdio: 'inherit',
				windowsHide: true,
			},
		);
		const hostPrefix = path.join(temporary, 'host');
		const hostArguments = ['build', '-Doptimize=ReleaseSafe', '--prefix', hostPrefix];
		hostArguments.push(...terminalHostBuildOptions(root, artifactStem));
		execFileSync('zig', hostArguments, {
			cwd: path.join(root, 'apps', 'terminal-host'),
			stdio: 'inherit',
			windowsHide: true,
		});
		const prepared = path.join(temporary, 'cohort');
		privateDirectory(prepared);
		const hashes = {};
		for (const artifact of [
			'orchbee',
			'orchbee-server',
			'orchbee-worker',
			'orchbee-terminal-host',
		]) {
			const sourceName = artifact + '.exe';
			// Cargo and Zig name the sources; the staged cohort carries the environment stem.
			const stagedName = artifactStem + artifact.slice('orchbee'.length) + '.exe';
			const source =
				artifact === 'orchbee-terminal-host'
					? path.join(hostPrefix, 'bin', sourceName)
					: path.join(target, 'release', sourceName);
			const staged = path.join(prepared, stagedName);
			copyFileSync(source, staged, constants.COPYFILE_EXCL);
			hashes[artifact] = createHash('sha256').update(readFileSync(staged)).digest('hex');
		}
		// A development or test cohort also carries the File Bar's process, which the Worker starts
		// against the digest `cohort.json` declares; a release cohort omits it until the File Bar ships.
		let filesDigest;
		if (artifactStem !== 'orchbee') {
			const staged = path.join(prepared, `${artifactStem}-files.exe`);
			copyFileSync(
				path.join(target, 'release', 'orchbee-files.exe'),
				staged,
				constants.COPYFILE_EXCL,
			);
			filesDigest = createHash('sha256').update(readFileSync(staged)).digest('hex');
			// The rg the File Bar's process searches with, and the notices of what rg carries.
			execFileSync(
				process.execPath,
				[
					path.join(root, 'scripts', 'ripgrep.mjs'),
					'--platform',
					process.arch === 'arm64' ? 'windows-aarch64' : 'windows-x86_64',
					'--output',
					prepared,
				],
				{ cwd: root, stdio: 'inherit', windowsHide: true },
			);
		}
		// The Windows launchers: the command launcher a release installs as `bin\<stem>.exe`, and
		// the window launcher it installs as the window entry `desktop\orchbee-desktop.exe`. Neither
		// is a cohort role, so `cohort.json` does not list them; the release manifest inventories
		// them like every file.
		for (const launcher of ['launcher', 'desktop-launcher'])
			copyFileSync(
				path.join(target, 'release', `orchbee-${launcher}.exe`),
				path.join(prepared, `${artifactStem}-${launcher}.exe`),
				constants.COPYFILE_EXCL,
			);
		// The notices of the third-party software the programs contain.
		writeProgramNotices({
			root,
			destination: prepared,
			packages: [
				'orchbee',
				'orchbee-server',
				'orchbee-worker',
				'orchbee-launcher',
				'orchbee-desktop-launcher',
				...(filesDigest === undefined ? [] : ['orchbee-files']),
			],
			platform: 'windows',
			development: filesDigest !== undefined,
		});
		const capsule = path.join(prepared, 'agent-observation-adapters');
		cpSync(
			path.join(root, 'crates', 'agent-integration-writer', 'agent-observation-adapters'),
			capsule,
			{
				errorOnExist: true,
				recursive: true,
				verbatimSymlinks: true,
			},
		);
		const digest = verifyCapsule(capsule);
		if (digest === null)
			throw new Error('Staged observation-adapters capsule failed canonical verification');
		hashes['agent-observation-adapters'] = digest;
		// The optional member follows the five required ones.
		if (filesDigest !== undefined) hashes['orchbee-files'] = filesDigest;
		execFileSync(
			process.execPath,
			[
				path.join(root, 'apps', 'terminal-host', 'scripts', 'windows-conpty.mjs'),
				'--output',
				path.join(prepared, 'conpty'),
			],
			{ cwd: root, stdio: 'inherit', windowsHide: true },
		);
		writeFileSync(
			path.join(prepared, 'cohort.json'),
			JSON.stringify({ schemaVersion: 1, artifacts: hashes }) + '\n',
			{ flag: 'wx' },
		);
		// Publish only a complete cohort. The temporary is on the same volume;
		// Windows refuses a rename over an existing destination directory.
		renameSync(prepared, output);
		process.stdout.write(
			'Staged Windows development cohort: ' +
				output +
				'\nRun in Windows Terminal: ' +
				path.join(output, artifactStem + '.exe') +
				'\n',
		);
	} finally {
		// This invocation owns the mkdtemp directory and every child has exited.
		removeOwnedTemporary(temporary, temporaryParent);
	}
}
