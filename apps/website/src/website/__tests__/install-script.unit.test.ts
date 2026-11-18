import { spawnSync } from 'node:child_process';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
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

interface PosixVerificationToolchain {
	readonly sh: string;
	readonly pathEnv: string;
}

/**
 * Finds a native POSIX `sh` and Ed25519-capable `openssl` without ever invoking WSL. On Windows,
 * uses Git for Windows's `usr\bin\sh.exe` and `usr\bin\openssl.exe`.
 */
function findPosixVerificationToolchain(): PosixVerificationToolchain | undefined {
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
			if (!existsSync(sh) || !existsSync(openssl)) continue;
			const help = spawnSync(openssl, ['pkeyutl', '-help'], {
				encoding: 'utf8',
				windowsHide: true,
			});
			if (`${help.stdout ?? ''}${help.stderr ?? ''}`.includes('-rawin')) {
				return { sh, pathEnv: `${usrBin};${currentPath}` };
			}
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
	if (probe.status !== 0) return undefined;
	return { sh, pathEnv: process.env['PATH'] ?? '/usr/bin:/bin' };
}

function toPosixPath(filename: string): string {
	const normalized = filename.replaceAll('\\', '/');
	const drive = /^([A-Za-z]):\/(.*)$/u.exec(normalized);
	if (drive?.[1] !== undefined && drive[2] !== undefined) {
		return `/${drive[1].toLowerCase()}/${drive[2]}`;
	}
	return normalized;
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
	toolchain: PosixVerificationToolchain,
	{
		feed,
		signature,
		testFeedEnv = 'file:///test/feed.json',
		locale = 'en_US.UTF-8',
	}: {
		readonly feed: Buffer;
		readonly signature: Buffer;
		readonly testFeedEnv?: string;
		readonly locale?: string;
	},
) {
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-install-sh-test-'));
	try {
		writeFileSync(path.join(scratch, 'feed.json'), feed);
		writeFileSync(path.join(scratch, 'feed.json.sig'), signature);
		const scriptSource = readFileSync(INSTALL_SCRIPT, 'utf8').replace(/\nob_main "\$@"\s*$/u, '\n');
		const harness = `${scriptSource}
ob_choose_locale
ob_tmp=${JSON.stringify(toPosixPath(scratch))}
ob_feed_url='https://orchbee.com/update/stable/macos-aarch64/feed.json'
ob_verify_feed
`;
		const baseEnv: NodeJS.ProcessEnv = {
			...process.env,
			LC_ALL: locale,
			ORCHBEE_INSTALL_SH_FEED: testFeedEnv,
			PATH: toolchain.pathEnv,
			...(process.platform === 'win32' ? { Path: toolchain.pathEnv } : {}),
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

const posixToolchain = findPosixVerificationToolchain();
// Only a bare developer Mac without Homebrew openssl@3 may lack Ed25519 pkeyutl -rawin; CI runners
// and Windows Git for Windows carry it.
const skipPosixVerification = posixToolchain === undefined && process.platform === 'darwin';

function requirePosixToolchain(): PosixVerificationToolchain {
	if (posixToolchain === undefined) {
		throw new Error('Expected POSIX sh and Ed25519-capable openssl on this host.');
	}
	return posixToolchain;
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

	it.skipIf(skipPosixVerification)(
		'admits valid release, test-mode and multi-key rotated signatures',
		() => {
			const toolchain = requirePosixToolchain();
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

	it.skipIf(skipPosixVerification)(
		'fails closed on tampered feeds, untrusted keys, test keys in production, wrong domain or malformed documents',
		() => {
			const toolchain = requirePosixToolchain();
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
});
