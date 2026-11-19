import { spawnSync } from 'node:child_process';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import {
	chmodSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const INSTALL_SCRIPT = fileURLToPath(new URL('../../../public/install.sh', import.meta.url));
const SIGNATURE_SOURCE = fileURLToPath(
	new URL('../../../../orchbee/src/cli/update/signature.rs', import.meta.url),
);
const FIXTURES_DIRECTORY = fileURLToPath(
	new URL('../../../../orchbee/src/cli/update/tests/fixtures/', import.meta.url),
);
const TRACKED_MACOS_FEED = fileURLToPath(
	new URL('../../../public/update/stable/macos-aarch64/feed.json.signed', import.meta.url),
);
const TRACKED_MACOS_SIG = fileURLToPath(
	new URL('../../../public/update/stable/macos-aarch64/feed.json.sig', import.meta.url),
);

function rustKeyTable(source: string, name: 'RELEASE_KEYS' | 'TEST_KEYS'): string[] {
	const table = new RegExp(`const ${name}: &\\[TrustedKey\\] = &\\[([\\s\\S]*?)\\];`, 'u').exec(
		source,
	);
	const body = (table?.[1] ?? '').replace(/\/\/[^\n]*/gu, '');
	return [...body.matchAll(/TrustedKey::from_hex\(\s*"([0-9a-f]{64})"\s*,?\s*\)/gu)].flatMap(
		(match) => (match[1] === undefined ? [] : [match[1]]),
	);
}

function shellKeyTable(script: string, variable: 'ob_release_keys' | 'ob_test_keys'): string[] {
	const match = new RegExp(`^${variable}='([\\s\\S]*?)'$`, 'mu').exec(script);
	const body = match?.[1] ?? '';
	return body
		.split(/\s+/u)
		.map((entry) => entry.trim())
		.filter((entry) => entry.length > 0);
}

interface PosixShellToolchain {
	readonly sh: string;
	readonly pathEnv: string;
	readonly hasEd25519Openssl: boolean;
}

/**
 * Finds a native POSIX `sh` without ever invoking WSL, and notes whether an Ed25519-capable
 * `openssl` is available alongside it. On Windows, uses Git for Windows's `usr\bin\sh.exe`.
 */
function findPosixShellToolchain(): PosixShellToolchain | undefined {
	const nodeDir = path.dirname(process.execPath);
	if (process.platform === 'win32') {
		const currentPath = process.env['Path'] ?? process.env['PATH'] ?? '';
		const pathEntries = currentPath.split(';');
		const candidates = new Set<string>();
		for (const entry of pathEntries) {
			const trimmed = entry.trim().replace(/^"(.*)"$/u, '$1');
			if (trimmed === '' || /system32|windowsapps|wsl/iu.test(trimmed)) continue;
			const gitCandidate = path.join(trimmed, 'git.exe');
			if (existsSync(gitCandidate)) {
				const gitRoot = path.dirname(path.dirname(gitCandidate));
				candidates.add(path.join(gitRoot, 'usr', 'bin'));
			}
		}
		const programFiles = process.env['ProgramFiles'];
		if (programFiles !== undefined) {
			candidates.add(path.join(programFiles, 'Git', 'usr', 'bin'));
		}
		for (const usrBin of candidates) {
			const sh = path.join(usrBin, 'sh.exe');
			const openssl = path.join(usrBin, 'openssl.exe');
			if (!existsSync(sh)) continue;
			let hasEd25519Openssl = false;
			if (existsSync(openssl)) {
				const help = spawnSync(openssl, ['pkeyutl', '-help'], {
					encoding: 'utf8',
					windowsHide: true,
				});
				hasEd25519Openssl = `${help.stdout ?? ''}${help.stderr ?? ''}`.includes('-rawin');
			}
			return { sh, pathEnv: `${usrBin};${nodeDir};${currentPath}`, hasEd25519Openssl };
		}
		return undefined;
	}

	const sh = '/bin/sh';
	if (!existsSync(sh)) return undefined;
	const probe = spawnSync(
		sh,
		[
			'-c',
			'for c in "$(command -v openssl 2>/dev/null)" /opt/homebrew/opt/openssl@3/bin/openssl /usr/local/opt/openssl@3/bin/openssl /opt/local/bin/openssl; do [ -n "$c" ] && [ -x "$c" ] && "$c" pkeyutl -help 2>&1 | grep -q -- -rawin && exit 0; done; exit 1',
		],
		{ encoding: 'utf8' },
	);
	const currentPath = process.env['PATH'] ?? '/usr/bin:/bin';
	return {
		sh,
		pathEnv: `${nodeDir}:${currentPath}`,
		hasEd25519Openssl: probe.status === 0,
	};
}

function toPosixPath(filename: string): string {
	const normalized = filename.replaceAll('\\', '/');
	const drive = /^([A-Za-z]):\/(.*)$/u.exec(normalized);
	if (drive?.[1] !== undefined && drive[2] !== undefined) {
		return `/${drive[1].toLowerCase()}/${drive[2]}`;
	}
	return normalized;
}

function prependPath(binDir: string, pathEnv: string): string {
	return process.platform === 'win32' ? `${binDir};${pathEnv}` : `${binDir}:${pathEnv}`;
}

function writeExecutableScript(filePath: string, body: string): void {
	writeFileSync(filePath, body, { encoding: 'utf8' });
	chmodSync(filePath, 0o755);
}

function signBytes(bytes: Buffer, domainPrefix: string) {
	const { publicKey, privateKey } = generateKeyPairSync('ed25519');
	const spki = publicKey.export({ format: 'der', type: 'spki' });
	const rawPublic = spki.subarray(spki.length - 32);
	const keyId = createHash('sha256').update(rawPublic).digest('hex').slice(0, 16);
	const signed = Buffer.concat([Buffer.from(`${domainPrefix}\n`, 'utf8'), bytes]);
	const signature = sign(null, signed, privateKey).toString('base64');
	return { keyId, algorithm: 'ed25519' as const, signature };
}

function runVerifyFeed(
	toolchain: PosixShellToolchain,
	{
		feed,
		signature,
		testFeedEnv = 'file:///test/feed.json',
		locale = 'en_US.UTF-8',
		verifier = 'openssl',
	}: {
		readonly feed: Buffer;
		readonly signature: Buffer;
		readonly testFeedEnv?: string;
		readonly locale?: string;
		readonly verifier?: 'openssl' | 'node';
	},
) {
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-install-sh-test-'));
	try {
		writeFileSync(path.join(scratch, 'feed.json'), feed);
		writeFileSync(path.join(scratch, 'feed.json.sig'), signature);
		let pathEnv = toolchain.pathEnv;
		if (verifier === 'node') {
			const stubBin = path.join(scratch, 'stub-bin');
			mkdirSync(stubBin, { recursive: true });
			// Simulates macOS LibreSSL `/usr/bin/openssl` whose `pkeyutl -help` lacks `-rawin`.
			writeExecutableScript(
				path.join(stubBin, 'openssl'),
				'#!/bin/sh\nif [ "${1:-}" = "pkeyutl" ]; then printf "Usage: pkeyutl [options]\\n"; exit 0; fi\nexit 1\n',
			);
			pathEnv = prependPath(stubBin, pathEnv);
		}
		const scriptSource = readFileSync(INSTALL_SCRIPT, 'utf8').replace(/\nob_main "\$@"\s*$/u, '\n');
		const harness = `${scriptSource}
ob_choose_locale
${verifier === 'node' ? "ob_openssl_paths=''" : ''}
ob_tmp=${JSON.stringify(toPosixPath(scratch))}
ob_feed_url='https://orchbee.com/update/stable/macos-aarch64/feed.json'
ob_verify_feed
`;
		const baseEnv: NodeJS.ProcessEnv = {
			...process.env,
			LC_ALL: locale,
			ORCHBEE_INSTALL_SH_FEED: testFeedEnv,
			PATH: pathEnv,
			...(process.platform === 'win32' ? { Path: pathEnv } : {}),
		};
		const result = spawnSync(toolchain.sh, ['-s'], {
			input: harness,
			encoding: 'utf8',
			env: baseEnv,
			timeout: 30_000,
			windowsHide: true,
		});
		return {
			status: result.status,
			stdout: result.stdout ?? '',
			stderr: result.stderr ?? '',
		};
	} finally {
		rmSync(scratch, { recursive: true, force: true });
	}
}

const posixToolchain = findPosixShellToolchain();
// Only a bare developer Mac without Homebrew openssl@3 lacks Ed25519 pkeyutl -rawin; CI runners and
// Windows Git for Windows carry it, while the Node fallback tests run everywhere POSIX sh exists.
const skipOpensslVerification =
	(posixToolchain === undefined || !posixToolchain.hasEd25519Openssl) &&
	process.platform === 'darwin';

function requirePosixShellToolchain(): PosixShellToolchain {
	if (posixToolchain === undefined) {
		throw new Error('Expected POSIX sh on this host.');
	}
	return posixToolchain;
}

function requireOpensslToolchain(): PosixShellToolchain {
	const toolchain = requirePosixShellToolchain();
	if (!toolchain.hasEd25519Openssl) {
		throw new Error('Expected Ed25519-capable openssl on this host.');
	}
	return toolchain;
}

describe('install.sh feed signature verification', () => {
	it('embeds the exact RELEASE_KEYS and TEST_KEYS from signature.rs', () => {
		const rustSource = readFileSync(SIGNATURE_SOURCE, 'utf8');
		const installScript = readFileSync(INSTALL_SCRIPT, 'utf8');
		const expectedRelease = rustKeyTable(rustSource, 'RELEASE_KEYS');
		const expectedTest = rustKeyTable(rustSource, 'TEST_KEYS');
		expect(expectedRelease.length).toBeGreaterThan(0);
		expect(expectedTest.length).toBeGreaterThan(0);
		expect(shellKeyTable(installScript, 'ob_release_keys')).toEqual(expectedRelease);
		expect(shellKeyTable(installScript, 'ob_test_keys')).toEqual(expectedTest);
	});

	it.skipIf(skipOpensslVerification)(
		'admits valid release, test-mode and multi-key rotated signatures',
		() => {
			const toolchain = requireOpensslToolchain();
			const fixtureFeed = readFileSync(path.join(FIXTURES_DIRECTORY, 'feed.json.signed'));
			const fixtureSig = readFileSync(path.join(FIXTURES_DIRECTORY, 'feed.json.sig'));

			const validTest = runVerifyFeed(toolchain, { feed: fixtureFeed, signature: fixtureSig });
			expect({ status: validTest.status, stderr: validTest.stderr }).toEqual({
				status: 0,
				stderr: '',
			});

			const prodFeed = readFileSync(TRACKED_MACOS_FEED);
			const prodSig = readFileSync(TRACKED_MACOS_SIG);
			const validProd = runVerifyFeed(toolchain, {
				feed: prodFeed,
				signature: prodSig,
				testFeedEnv: '',
			});
			expect({ status: validProd.status, stderr: validProd.stderr }).toEqual({
				status: 0,
				stderr: '',
			});

			const unknownEntry = signBytes(fixtureFeed, 'orchbee-update-feed-v1');
			const parsedFixture = JSON.parse(fixtureSig.toString('utf8')) as {
				signatures: { keyId: string; algorithm: string; signature: string }[];
			};
			const trustedEntry = parsedFixture.signatures[0];
			expect(trustedEntry).toBeDefined();
			const rotatedSig = Buffer.from(
				JSON.stringify({
					signatures: [
						unknownEntry,
						{
							signature: trustedEntry?.signature,
							algorithm: 'ed25519',
							keyId: trustedEntry?.keyId,
						},
					],
				}) + '\n',
				'utf8',
			);
			const validRotated = runVerifyFeed(toolchain, { feed: fixtureFeed, signature: rotatedSig });
			expect({ status: validRotated.status, stderr: validRotated.stderr }).toEqual({
				status: 0,
				stderr: '',
			});
		},
		20_000,
	);

	it.skipIf(skipOpensslVerification)(
		'fails closed on tampered feeds, untrusted keys, test keys in production, wrong domain or malformed documents',
		() => {
			const toolchain = requireOpensslToolchain();
			const fixtureFeed = readFileSync(path.join(FIXTURES_DIRECTORY, 'feed.json.signed'));
			const fixtureSig = readFileSync(path.join(FIXTURES_DIRECTORY, 'feed.json.sig'));
			const parsedFixture = JSON.parse(fixtureSig.toString('utf8')) as {
				signatures: { keyId: string; algorithm: string; signature: string }[];
			};
			const trustedEntry = parsedFixture.signatures[0];
			expect(trustedEntry).toBeDefined();

			const testKeyInProd = runVerifyFeed(toolchain, {
				feed: fixtureFeed,
				signature: fixtureSig,
				testFeedEnv: '',
			});
			expect(testKeyInProd.status).toBe(1);
			expect(testKeyInProd.stderr).toContain('Nothing was installed.');

			const tamperedFeed = Buffer.from(
				fixtureFeed.toString('utf8').replace('"fixture.1"', '"fixture.2"'),
				'utf8',
			);
			const tamperedEn = runVerifyFeed(toolchain, { feed: tamperedFeed, signature: fixtureSig });
			expect(tamperedEn.status).toBe(1);
			expect(tamperedEn.stderr).toContain('Nothing was installed.');

			const tamperedZh = runVerifyFeed(toolchain, {
				feed: tamperedFeed,
				signature: fixtureSig,
				locale: 'zh_CN.UTF-8',
			});
			expect(tamperedZh.status).toBe(1);
			expect(tamperedZh.stderr).toContain('未安装任何内容。');

			const untrustedSig = Buffer.from(
				JSON.stringify({ signatures: [signBytes(fixtureFeed, 'orchbee-update-feed-v1')] }) + '\n',
				'utf8',
			);
			const untrusted = runVerifyFeed(toolchain, { feed: fixtureFeed, signature: untrustedSig });
			expect(untrusted.status).toBe(1);
			expect(untrusted.stderr).toContain('Nothing was installed.');

			const wrongDomainSig = Buffer.from(
				JSON.stringify({
					signatures: [
						{
							keyId: trustedEntry?.keyId,
							algorithm: 'ed25519',
							signature: signBytes(fixtureFeed, 'orchbee-release-manifest-v2').signature,
						},
					],
				}) + '\n',
				'utf8',
			);
			const wrongDomain = runVerifyFeed(toolchain, {
				feed: fixtureFeed,
				signature: wrongDomainSig,
			});
			expect(wrongDomain.status).toBe(1);

			for (const badSig of [
				Buffer.from('{"signatures":[]}\n'),
				Buffer.from(`{"signatures":[${JSON.stringify(trustedEntry)}],"extra":1}\n`),
				Buffer.from(' '.repeat(4097)),
			]) {
				const malformed = runVerifyFeed(toolchain, { feed: fixtureFeed, signature: badSig });
				expect(malformed.status).toBe(1);
				expect(malformed.stderr).toContain('Nothing was installed.');
			}
		},
		20_000,
	);

	it('falls back to Node.js Ed25519 verification when OpenSSL lacks pkeyutl -rawin', () => {
		const toolchain = requirePosixShellToolchain();
		const fixtureFeed = readFileSync(path.join(FIXTURES_DIRECTORY, 'feed.json.signed'));
		const fixtureSig = readFileSync(path.join(FIXTURES_DIRECTORY, 'feed.json.sig'));
		const prodFeed = readFileSync(TRACKED_MACOS_FEED);
		const prodSig = readFileSync(TRACKED_MACOS_SIG);

		const validProd = runVerifyFeed(toolchain, {
			feed: prodFeed,
			signature: prodSig,
			testFeedEnv: '',
			verifier: 'node',
		});
		expect({ status: validProd.status, stderr: validProd.stderr }).toEqual({
			status: 0,
			stderr: '',
		});

		const validTest = runVerifyFeed(toolchain, {
			feed: fixtureFeed,
			signature: fixtureSig,
			verifier: 'node',
		});
		expect({ status: validTest.status, stderr: validTest.stderr }).toEqual({
			status: 0,
			stderr: '',
		});

		const tamperedFeed = Buffer.from(
			fixtureFeed.toString('utf8').replace('"fixture.1"', '"fixture.2"'),
			'utf8',
		);
		const tampered = runVerifyFeed(toolchain, {
			feed: tamperedFeed,
			signature: fixtureSig,
			verifier: 'node',
		});
		expect(tampered.status).toBe(1);
		expect(tampered.stderr).toContain('Nothing was installed.');

		const untrustedSig = Buffer.from(
			JSON.stringify({ signatures: [signBytes(fixtureFeed, 'orchbee-update-feed-v1')] }) + '\n',
			'utf8',
		);
		const untrusted = runVerifyFeed(toolchain, {
			feed: fixtureFeed,
			signature: untrustedSig,
			verifier: 'node',
		});
		expect(untrusted.status).toBe(1);
		expect(untrusted.stderr).toContain('Nothing was installed.');

		const testKeyInProd = runVerifyFeed(toolchain, {
			feed: fixtureFeed,
			signature: fixtureSig,
			testFeedEnv: '',
			verifier: 'node',
		});
		expect(testKeyInProd.status).toBe(1);
		expect(testKeyInProd.stderr).toContain('Nothing was installed.');
	}, 20_000);
});

function createFakeAppBundle(rootDir: string, version: string, helperSupported = true): void {
	const cohortDir = path.join(rootDir, 'Orchbee.app', 'Contents', 'Resources', 'cohort');
	mkdirSync(cohortDir, { recursive: true });
	writeFileSync(
		path.join(cohortDir, 'release.json'),
		JSON.stringify({ version, buildId: `${version}.build` }) + '\n',
		'utf8',
	);
	writeFileSync(
		path.join(rootDir, 'Orchbee.app', 'Contents', 'Info.plist'),
		`<plist><string>${version}</string></plist>\n`,
		'utf8',
	);
	writeExecutableScript(
		path.join(cohortDir, 'orchbee'),
		helperSupported
			? '#!/bin/sh\nprintf \'{"schema": "orchbee.install", "version": 1, "result": {"activated": true}}\\n\'\nexit 0\n'
			: '#!/bin/sh\nexit 2\n',
	);
}

function runInstallDesktop(
	toolchain: PosixShellToolchain,
	{
		existingVersion,
		newVersion = '0.2.0',
		running,
		sessionRunning = false,
		quitBehavior = 'exits',
		helperSupported = true,
		locale = 'en_US.UTF-8',
	}: {
		readonly existingVersion: string | null;
		readonly newVersion?: string;
		readonly running: boolean;
		readonly sessionRunning?: boolean;
		readonly quitBehavior?: 'exits' | 'stays-running' | 'hangs';
		readonly helperSupported?: boolean;
		readonly locale?: string;
	},
) {
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-install-desktop-test-'));
	try {
		const stubBin = path.join(scratch, 'stub-bin');
		const appsDir = path.join(scratch, 'Applications');
		const workTmp = path.join(scratch, 'tmp');
		const archiveBuild = path.join(scratch, 'archive-src');
		const windowFolder = `orchbee-desktop-${newVersion}-macos-aarch64`;
		const windowName = `${windowFolder}.zip`;
		mkdirSync(stubBin, { recursive: true });
		mkdirSync(appsDir, { recursive: true });
		mkdirSync(workTmp, { recursive: true });
		mkdirSync(path.join(archiveBuild, windowFolder), { recursive: true });

		createFakeAppBundle(path.join(archiveBuild, windowFolder), newVersion, helperSupported);
		if (existingVersion !== null) {
			createFakeAppBundle(appsDir, existingVersion, true);
		}
		const runningMarkerPosix = toPosixPath(path.join(scratch, 'bundle-running'));
		const osascriptLogPosix = toPosixPath(path.join(scratch, 'osascript.log'));
		const openLogPosix = toPosixPath(path.join(scratch, 'open.log'));
		const expectedTargetPosix = `${toPosixPath(appsDir)}/Orchbee.app`;
		const runningProcesses = [
			...(running ? [`${expectedTargetPosix}/Contents/MacOS/Orchbee`] : []),
			...(sessionRunning ? [`${expectedTargetPosix}/Contents/Resources/cohort/orchbee`] : []),
		];
		if (runningProcesses.length > 0) {
			writeFileSync(path.join(scratch, 'bundle-running'), `${runningProcesses.join('\n')}\n`);
		}

		writeExecutableScript(
			path.join(stubBin, 'ditto'),
			'#!/bin/sh\nrm -rf "$2" && cp -R "$1" "$2"\n',
		);
		writeExecutableScript(
			path.join(stubBin, 'codesign'),
			'#!/bin/sh\n[ -d "$4" ] && [ -f "$4/Contents/Resources/cohort/release.json" ]\n',
		);
		writeExecutableScript(
			path.join(stubBin, 'pgrep'),
			`#!/bin/sh
[ -f ${JSON.stringify(runningMarkerPosix)} ] || exit 1
pattern=''
while [ $# -gt 0 ]; do
	case $1 in
	-u) shift ;;
	-f) shift; pattern=$1 ;;
	*) pattern=$1 ;;
	esac
	shift
done
grep -E -- "$pattern" ${JSON.stringify(runningMarkerPosix)} >/dev/null 2>&1
`,
		);
		writeExecutableScript(
			path.join(stubBin, 'osascript'),
			`#!/bin/sh
target=''
for arg in "$@"; do
	target=$arg
done
printf '%s\\n' "$target" >>${JSON.stringify(osascriptLogPosix)}
${quitBehavior === 'exits' ? `rm -f ${JSON.stringify(runningMarkerPosix)}` : ':'}
${quitBehavior === 'hangs' ? 'exec sleep 30' : 'exit 0'}
`,
		);
		writeExecutableScript(
			path.join(stubBin, 'open'),
			`#!/bin/sh\nprintf '%s\\n' "$1" >>${JSON.stringify(openLogPosix)}\nexit 0\n`,
		);

		const scriptSource = readFileSync(INSTALL_SCRIPT, 'utf8').replace(/\nob_main "\$@"\s*$/u, '\n');
		const harness = `${scriptSource}
ob_choose_locale
ob_tmp=${JSON.stringify(toPosixPath(workTmp))}
ob_stem='orchbee'
ob_window_version=${JSON.stringify(newVersion)}
ob_window_folder=${JSON.stringify(windowFolder)}
ob_window_name=${JSON.stringify(windowName)}
ob_quit_polls=2
ob_quit_poll_sleep=0.01
ob_quit_check_sleep=0.01
${quitBehavior === 'hangs' ? 'ob_quit_timeout=1' : ''}
tar -c -f "$ob_tmp/$ob_window_name" -C ${JSON.stringify(toPosixPath(archiveBuild))} "$ob_window_folder"
ob_install_desktop
`;
		const pathEnv = prependPath(stubBin, toolchain.pathEnv);
		const env: NodeJS.ProcessEnv = {
			...process.env,
			LC_ALL: locale,
			ORCHBEE_INSTALL_SH_APPLICATIONS: toPosixPath(appsDir),
			PATH: pathEnv,
			...(process.platform === 'win32' ? { Path: pathEnv } : {}),
		};
		const result = spawnSync(toolchain.sh, ['-s'], {
			input: harness,
			encoding: 'utf8',
			env,
			timeout: 30_000,
			windowsHide: true,
		});
		const installedRelease = JSON.parse(
			readFileSync(
				path.join(appsDir, 'Orchbee.app', 'Contents', 'Resources', 'cohort', 'release.json'),
				'utf8',
			),
		) as { version: string };
		const readLog = (name: string) =>
			existsSync(path.join(scratch, name))
				? readFileSync(path.join(scratch, name), 'utf8').trim().split('\n').filter(Boolean)
				: [];
		return {
			status: result.status,
			stdout: result.stdout ?? '',
			stderr: result.stderr ?? '',
			installedVersion: installedRelease.version,
			appsEntries: readdirSync(appsDir).sort(),
			osascriptTargets: readLog('osascript.log'),
			openTargets: readLog('open.log'),
			expectedTargetPosix,
		};
	} finally {
		rmSync(scratch, { recursive: true, force: true });
	}
}

describe('install.sh --desktop live hot-swap', () => {
	it('leaves a running bundle untouched when it already runs the exact release', () => {
		const toolchain = requirePosixShellToolchain();
		const en = runInstallDesktop(toolchain, {
			existingVersion: '0.2.0',
			newVersion: '0.2.0',
			running: true,
		});
		expect(en.status).toBe(0);
		expect(en.stderr).toBe('');
		expect(en.osascriptTargets).toEqual([]);
		expect(en.openTargets).toEqual([]);
		expect(en.installedVersion).toBe('0.2.0');
	});

	it('stages first, quits an older running bundle gracefully, swaps it and reopens it', () => {
		const toolchain = requirePosixShellToolchain();
		const updated = runInstallDesktop(toolchain, {
			existingVersion: '0.1.0',
			newVersion: '0.2.0',
			running: true,
			quitBehavior: 'exits',
		});
		expect(updated.status).toBe(0);
		expect(updated.stderr).toBe('');
		expect(updated.osascriptTargets).toEqual([updated.expectedTargetPosix]);
		expect(updated.openTargets).toEqual([updated.expectedTargetPosix]);
		expect(updated.installedVersion).toBe('0.2.0');
		expect(updated.appsEntries).toEqual(['Orchbee.app']);
	});

	it('leaves the bundle in place and asks no closed window to quit when only a Session runs from it', () => {
		const toolchain = requirePosixShellToolchain();
		const staged = runInstallDesktop(toolchain, {
			existingVersion: '0.1.0',
			newVersion: '0.2.0',
			running: false,
			sessionRunning: true,
			helperSupported: true,
		});
		expect(staged.status).toBe(0);
		expect(staged.stderr).toBe('');
		expect(staged.osascriptTargets).toEqual([]);
		expect(staged.openTargets).toEqual([]);
		expect(staged.installedVersion).toBe('0.1.0');
		expect(staged.appsEntries).toEqual(['Orchbee.app']);

		const withoutStore = runInstallDesktop(toolchain, {
			existingVersion: '0.1.0',
			newVersion: '0.2.0',
			running: false,
			sessionRunning: true,
			helperSupported: false,
		});
		expect(withoutStore.status).toBe(1);
		expect(withoutStore.osascriptTargets).toEqual([]);
		expect(withoutStore.installedVersion).toBe('0.1.0');
		expect(withoutStore.appsEntries).toEqual(['Orchbee.app']);
	});

	it('keeps the bundle and fails when the open window refuses to quit and the store lacks the version', () => {
		const toolchain = requirePosixShellToolchain();
		const refused = runInstallDesktop(toolchain, {
			existingVersion: '0.1.0',
			newVersion: '0.2.0',
			running: true,
			quitBehavior: 'stays-running',
			helperSupported: false,
		});
		expect(refused.status).toBe(1);
		expect(refused.osascriptTargets).toEqual([refused.expectedTargetPosix]);
		expect(refused.openTargets).toEqual([]);
		expect(refused.installedVersion).toBe('0.1.0');
		expect(refused.appsEntries).toEqual(['Orchbee.app']);
	});

	it('does not wait on a quit request that never returns and ends like the refusing window', () => {
		const toolchain = requirePosixShellToolchain();
		const hung = runInstallDesktop(toolchain, {
			existingVersion: '0.1.0',
			newVersion: '0.2.0',
			running: true,
			quitBehavior: 'hangs',
			helperSupported: true,
		});
		expect(hung.status).toBe(0);
		expect(hung.stderr).toBe('');
		expect(hung.osascriptTargets).toEqual([hung.expectedTargetPosix]);
		expect(hung.openTargets).toEqual([]);
		expect(hung.installedVersion).toBe('0.1.0');
		expect(hung.appsEntries).toEqual(['Orchbee.app']);
	});
});
