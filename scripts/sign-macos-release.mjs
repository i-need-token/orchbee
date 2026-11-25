// The macOS release jobs use this entry point after building, with no npm dependencies.
// Code signing precedes the native manifest. Notarization precedes the desktop file list.
import { execFileSync, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import {
	chmodSync,
	closeSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	openSync,
	readFileSync,
	readdirSync,
	readSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { macDiskImageName, writeMacDiskImage } from '../apps/desktop/scripts/mac-disk-image.mjs';
import { hostReleasePlatform, releaseFiles } from './package-native-release.mjs';

const security = '/usr/bin/security';
const codesign = '/usr/bin/codesign';
const xcrun = '/usr/bin/xcrun';
const profile = 'orchbee-release';
const submissionFile = 'notary-submission-id';
const peers = ['orchbee', 'orchbee-server', 'orchbee-worker', 'orchbee-terminal-host'];
const programs = [...peers, 'orchbee-desktop-launcher'];
const machOMagic = new Set([
	0xfeedface, 0xcefaedfe, 0xfeedfacf, 0xcffaedfe, 0xcafebabe, 0xbebafeca, 0xcafebabf, 0xbfbafeca,
]);
// Electron's main and helper processes run V8 JIT code under Hardened Runtime.
const electronEntitlements = `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>com.apple.security.cs.allow-jit</key><true/></dict></plist>
`;

function takeCredential(name) {
	const value = process.env[name];
	delete process.env[name];
	if (!value || ['\0', '\r', '\n'].some((character) => value.includes(character)))
		throw new Error(`Missing or invalid ${name}`);
	return value;
}

// security's interactive mode reads the commands on stdin. Keep passwords out of argv and discard
// all output from these commands, including usage diagnostics that might repeat an input word.
function keychainCommand(command, ...words) {
	const input = [command, ...words.map((word) => '"' + word.replaceAll(/[\\"]/gu, '\\$&') + '"')];
	const result = spawnSync(security, ['-i'], { input: input.join(' ') + '\n', encoding: 'utf8' });
	if (result.status !== 0 || result.stderr.trim() !== '')
		throw new Error(`security ${command} failed`);
}

async function withKeychain(action) {
	const workspace = mkdtempSync(path.join(tmpdir(), 'orchbee-apple-'));
	const keychain = path.join(workspace, 'release.keychain-db');
	const password = randomBytes(32).toString('hex');
	try {
		keychainCommand('create-keychain', '-p', password, keychain);
		keychainCommand('unlock-keychain', '-p', password, keychain);
		execFileSync(security, ['set-keychain-settings', '-lut', '3600', keychain]);
		return await action({ workspace, keychain, password });
	} finally {
		try {
			if (existsSync(keychain))
				execFileSync(security, ['delete-keychain', keychain], { stdio: 'ignore' });
		} finally {
			rmSync(workspace, { force: true, recursive: true });
		}
	}
}

function developerIdentities(keychain) {
	const output = execFileSync(security, ['find-identity', '-v', '-p', 'codesigning', keychain], {
		encoding: 'utf8',
	});
	return [
		...output.matchAll(
			/^\s*\d+\) ([A-F0-9]{40}) "Developer ID Application: .+ \(([A-Z0-9]{10})\)"$/gmu,
		),
	];
}

async function withDeveloperIdentity(action) {
	const team = takeCredential('APPLE_TEAM_ID');
	const certificate = takeCredential('MACOS_SIGNING_CERTIFICATE_BASE64');
	const password = takeCredential('MACOS_SIGNING_CERTIFICATE_PASSWORD');
	await withKeychain(async ({ workspace, keychain, password: keychainPassword }) => {
		const p12 = path.join(workspace, 'identity.p12');
		writeFileSync(p12, Buffer.from(certificate, 'base64'), { mode: 0o600 });
		keychainCommand('import', p12, '-f', 'pkcs12', '-k', keychain, '-P', password, '-T', codesign);
		rmSync(p12);
		// codesign needs to build the certificate chain as well as locate the signing identity.
		// Deleting the temporary keychain removes its entry from the user's search list.
		const searchList = execFileSync(security, ['list-keychains', '-d', 'user'], {
			encoding: 'utf8',
		});
		const keychains = [...searchList.matchAll(/^\s*"(.+)"$/gmu)].map((entry) => entry[1]);
		execFileSync(security, ['list-keychains', '-d', 'user', '-s', keychain, ...keychains]);
		let identities = developerIdentities(keychain);
		if (identities.length === 0) {
			// The exported identity need not include the public Developer ID G2 intermediate.
			const response = await fetch(
				'https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer',
				{
					signal: AbortSignal.timeout(30000),
				},
			);
			if (!response.ok) throw new Error('Could not download the Apple Developer ID intermediate');
			const intermediate = path.join(workspace, 'DeveloperIDG2CA.cer');
			writeFileSync(intermediate, Buffer.from(await response.arrayBuffer()));
			execFileSync(security, ['import', intermediate, '-k', keychain]);
			identities = developerIdentities(keychain);
		}
		if (identities.length !== 1 || identities[0][2] !== team)
			throw new Error('Expected one valid Developer ID Application identity for APPLE_TEAM_ID');
		keychainCommand(
			'set-key-partition-list',
			'-S',
			'apple-tool:,apple:',
			'-s',
			'-k',
			keychainPassword,
			keychain,
		);
		const identity = identities[0][1];
		function sign(filename, options = []) {
			execFileSync(
				codesign,
				[
					'--force',
					'--sign',
					identity,
					'--keychain',
					keychain,
					'--timestamp',
					...options,
					filename,
				],
				{ stdio: 'inherit' },
			);
		}
		await action(sign, workspace);
	});
}

async function signNative(cohort) {
	const files = new Map(releaseFiles(cohort).map((file) => [file.path, file.sha256]));
	const manifestPath = path.join(cohort, 'cohort.json');
	const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
	for (const peer of peers)
		if (!files.has(peer) || files.get(peer) !== manifest.artifacts?.[peer])
			throw new Error(`The staged native cohort does not match ${peer}`);
	if (!files.has(programs.at(-1)))
		throw new Error('The staged native cohort has no desktop launcher');
	await withDeveloperIdentity((sign) => {
		for (const program of programs) {
			const filename = path.join(cohort, program);
			chmodSync(filename, 0o755);
			execFileSync('/usr/bin/lipo', [
				filename,
				'-verify_arch',
				process.arch === 'arm64' ? 'arm64' : 'x86_64',
			]);
			sign(filename, ['--options', 'runtime']);
			execFileSync(codesign, ['--verify', '--strict', filename], { stdio: 'inherit' });
		}
	});
	const signed = new Map(releaseFiles(cohort).map((file) => [file.path, file.sha256]));
	for (const peer of peers) manifest.artifacts[peer] = signed.get(peer);
	writeFileSync(manifestPath, JSON.stringify(manifest) + '\n');
}

function isMachO(filename) {
	const fd = openSync(filename, 'r');
	try {
		const magic = Buffer.alloc(4);
		return readSync(fd, magic, 0, 4, 0) === 4 && machOMagic.has(magic.readUInt32BE());
	} finally {
		closeSync(fd);
	}
}

function desktopPaths(root, version, platform) {
	const applicationDirectory = path.join(root, `Orchbee-darwin-${process.arch}`);
	const application = path.join(applicationDirectory, 'Orchbee.app');
	const cohort = path.join(application, 'Contents', 'Resources', 'cohort');
	const image = path.join(root, 'installer', macDiskImageName('Orchbee', version, platform));
	const manifest = JSON.parse(readFileSync(path.join(cohort, 'release.json'), 'utf8'));
	if (manifest.version !== version || manifest.platform !== platform)
		throw new Error('The desktop cohort does not match the release version and platform');
	return { application, applicationDirectory, cohort, image };
}

async function signDesktop(root, version, platform) {
	const { application, cohort, image } = desktopPaths(root, version, platform);
	const originalCohort = JSON.stringify(releaseFiles(cohort));
	await withDeveloperIdentity(async (sign, workspace) => {
		const entitlements = path.join(workspace, 'electron-entitlements.plist');
		writeFileSync(entitlements, electronEntitlements);
		// Seal nested code inside out. The native cohort is already signed and its release.json
		// binds its bytes. Electron's framework links refer to files reached by the physical walk.
		function seal(directory) {
			for (const entry of readdirSync(directory, { withFileTypes: true })) {
				const filename = path.join(directory, entry.name);
				if (filename === cohort) continue;
				if (entry.isDirectory()) seal(filename);
				else if (entry.isFile() && isMachO(filename)) sign(filename, ['--options', 'runtime']);
			}
			const extension = path.extname(directory);
			if (extension === '.app')
				sign(directory, ['--options', 'runtime', '--entitlements', entitlements]);
			else if (extension === '.framework' || extension === '.xpc')
				sign(directory, ['--options', 'runtime']);
		}
		seal(application);
		execFileSync(codesign, ['--verify', '--deep', '--strict', application], { stdio: 'inherit' });
		if (originalCohort !== JSON.stringify(releaseFiles(cohort)))
			throw new Error('Desktop code signing changed the signed native cohort');
		mkdirSync(path.dirname(image));
		await writeMacDiskImage({ application, volumeName: 'Orchbee', image, workspace });
		sign(image);
		execFileSync(codesign, ['--verify', '--strict', image], { stdio: 'inherit' });
	});
}

// notarytool prompts for these credentials in a TTY. Expect supplies them without echoing or using
// arguments. Its child receives neither the credential variables nor a transcript of the prompts.
const notaryCredentials = String.raw`
set timeout 120
log_user 0
set appleId $env(APPLE_ID)
set team $env(APPLE_TEAM_ID)
set password $env(APPLE_APP_SPECIFIC_PASSWORD)
unset env(APPLE_ID) env(APPLE_TEAM_ID) env(APPLE_APP_SPECIFIC_PASSWORD)
proc answer {prompt value} {
    expect {
        -re $prompt { send -- "$value\r" }
        timeout { exit 10 }
        eof { exit 11 }
    }
}
spawn -noecho /usr/bin/xcrun notarytool store-credentials $env(ORCHBEE_NOTARY_PROFILE) --keychain $env(ORCHBEE_NOTARY_KEYCHAIN)
answer {Path to App Store Connect API private key:} ""
answer {Developer Apple ID:} $appleId
answer {App-specific password[^:]*:} $password
answer {Developer Team ID:} $team
set httpStatus ""
expect {
    -re {HTTP status code: ([0-9]{3})} {
        set httpStatus $expect_out(1,string)
        exp_continue -continue_timer
    }
    eof {
        if {$httpStatus ne ""} { puts stderr $httpStatus }
        exit [lindex [wait] 3]
    }
    timeout { exit 12 }
}
`;

async function withNotaryCredentials(action) {
	const credentials = Object.fromEntries(
		['APPLE_ID', 'APPLE_TEAM_ID', 'APPLE_APP_SPECIFIC_PASSWORD'].map((name) => [
			name,
			takeCredential(name),
		]),
	);
	return withKeychain(({ keychain }) => {
		const stored = spawnSync('/usr/bin/expect', [], {
			input: notaryCredentials,
			env: {
				...process.env,
				...credentials,
				ORCHBEE_NOTARY_PROFILE: profile,
				ORCHBEE_NOTARY_KEYCHAIN: keychain,
			},
			timeout: 180000,
		});
		if (stored.status !== 0) {
			const httpStatus = stored.stderr?.toString().trim() ?? '';
			const reason = /^\d{3}$/u.test(httpStatus) ? `HTTP ${httpStatus}` : `exit ${stored.status}`;
			throw new Error(`notarytool could not validate and store the Apple credentials (${reason})`);
		}
		return action(['--keychain-profile', profile, '--keychain', keychain]);
	});
}

function notarizationId(value) {
	if (typeof value !== 'string' || !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/iu.test(value))
		throw new Error('Expected an Apple notarization submission identifier');
	return value;
}

async function submitDesktop(root, version, platform) {
	const { image } = desktopPaths(root, version, platform);
	const receipt = path.join(root, submissionFile);
	if (existsSync(receipt))
		throw new Error(`The desktop already has a submission in ${submissionFile}`);
	await withNotaryCredentials((authentication) => {
		const response = execFileSync(
			xcrun,
			['notarytool', 'submit', image, ...authentication, '--no-wait', '--output-format', 'json'],
			{ encoding: 'utf8' },
		);
		const id = notarizationId(JSON.parse(response).id);
		console.info(`Apple notarization submission ${id}`);
		writeFileSync(receipt, id + '\n', { flag: 'wx' });
	});
}

async function notarizeDesktop(root, version, platform) {
	const { application, applicationDirectory, image } = desktopPaths(root, version, platform);
	const receipt = path.join(root, submissionFile);
	const id = notarizationId(readFileSync(receipt, 'utf8').trim());
	await withNotaryCredentials((authentication) => {
		console.info(`Waiting for Apple notarization submission ${id}`);
		const result = spawnSync(
			xcrun,
			['notarytool', 'wait', id, ...authentication, '--timeout', '50m', '--output-format', 'json'],
			{ encoding: 'utf8' },
		);
		const submission = result.stdout?.trim().startsWith('{') ? JSON.parse(result.stdout) : {};
		console.info(`Apple notarization submission ${id}: ${submission.status ?? 'wait interrupted'}`);
		if (result.status !== 0 || submission.status !== 'Accepted') {
			if (['Invalid', 'Rejected'].includes(submission.status))
				execFileSync(xcrun, ['notarytool', 'log', id, ...authentication], {
					stdio: 'inherit',
				});
			throw new Error(
				result.stderr?.trim() || `Apple has not accepted notarization submission ${id}`,
			);
		}
	});
	// Notarizing the DMG also issues a ticket for the contained application.
	for (const artifact of [image, application]) {
		execFileSync(xcrun, ['stapler', 'staple', artifact], { stdio: 'inherit' });
		execFileSync(xcrun, ['stapler', 'validate', artifact], { stdio: 'inherit' });
	}
	execFileSync(
		process.execPath,
		[
			fileURLToPath(new URL('./desktop-files.mjs', import.meta.url)),
			'write',
			applicationDirectory,
			version,
			path.basename(application),
		],
		{ stdio: 'inherit' },
	);
	rmSync(receipt);
}

const [command, root, platform, version, ...rest] = process.argv.slice(2);
try {
	if (
		process.platform !== 'darwin' ||
		!root ||
		!path.isAbsolute(root) ||
		platform !== hostReleasePlatform() ||
		rest.length > 0
	)
		throw new Error('Expected an absolute release directory and this Mac host’s release platform');
	if (command === 'native' && version === undefined) await signNative(root);
	else if (command === 'desktop' && version) await signDesktop(root, version, platform);
	else if (command === 'submit' && version) await submitDesktop(root, version, platform);
	else if (command === 'notarize' && version) await notarizeDesktop(root, version, platform);
	else
		throw new Error(
			'Usage: sign-macos-release.mjs native|desktop|submit|notarize <directory> <platform> [version]',
		);
} catch (error) {
	console.error(error.message);
	process.exitCode = 1;
}
