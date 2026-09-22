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
	writeFileSync,
} from 'node:fs';
import path from 'node:path';

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

export function stageWindows(root, output, cargoTarget, verifyCapsule) {
	if (
		!/^[A-Za-z]:\\/u.test(output) ||
		path.normalize(output) !== output ||
		path.parse(output).root === output
	)
		throw new Error('Output must be a normalized new absolute directory');
	if (existsSync(output)) throw new Error('Refusing to overwrite an existing cohort');
	const target = cargoTarget ?? path.join(root, 'target');
	if (!path.isAbsolute(target)) throw new Error('Cargo target directory must be absolute');
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
		],
		{ cwd: root, stdio: 'inherit', windowsHide: true },
	);
	const temporaryParent = realpathSync(path.dirname(output));
	const temporary = mkdtempSync(path.join(temporaryParent, 'orchbee-native-host-'));
	try {
		const hostPrefix = path.join(temporary, 'host');
		execFileSync('zig', ['build', '-Doptimize=ReleaseSafe', '--prefix', hostPrefix], {
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
			const filename = artifact + '.exe';
			const source =
				artifact === 'orchbee-terminal-host'
					? path.join(hostPrefix, 'bin', filename)
					: path.join(target, 'release', filename);
			const staged = path.join(prepared, filename);
			copyFileSync(source, staged, constants.COPYFILE_EXCL);
			hashes[artifact] = createHash('sha256').update(readFileSync(staged)).digest('hex');
		}
		const capsule = path.join(prepared, 'agent-observation-adapters');
		cpSync(path.join(root, 'apps', 'desktop', 'resources', 'agent-observation-adapters'), capsule, {
			errorOnExist: true,
			recursive: true,
			verbatimSymlinks: true,
		});
		const digest = verifyCapsule(capsule);
		if (digest === null)
			throw new Error('Staged observation-adapters capsule failed canonical verification');
		hashes['agent-observation-adapters'] = digest;
		writeFileSync(
			path.join(prepared, 'cohort.json'),
			JSON.stringify({ schemaVersion: 2, artifacts: hashes }) + '\n',
			{ flag: 'wx' },
		);
		// Publish only a complete cohort. The temporary is on the same volume;
		// Windows refuses a rename over an existing destination directory.
		renameSync(prepared, output);
		process.stdout.write(
			'Staged Windows development cohort: ' +
				output +
				'\nRun in Windows Terminal: ' +
				path.join(output, 'orchbee.exe') +
				'\n',
		);
	} finally {
		// This invocation owns the mkdtemp directory and every child has exited.
		removeOwnedTemporary(temporary, temporaryParent);
	}
}
