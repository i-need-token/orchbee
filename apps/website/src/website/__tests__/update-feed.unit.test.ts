import { createHash, createPublicKey, verify } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { release } from '../release';

// Served as /update/<channel>/<platform>/feed.json and feed.json.sig; the build renames
// feed.json.signed, which holds the exact signed bytes.
const UPDATE_DIRECTORY = new URL('../../../public/update/', import.meta.url);
const SIGNATURE_SOURCE = new URL(
	'../../../../orchbee/src/cli/update/signature.rs',
	import.meta.url,
);
const FEED_DOMAIN = Buffer.from('orchbee-update-feed-v1\n', 'utf8');
const ED25519_SPKI = Buffer.from('302a300506032b6570032100', 'hex');

const SignatureDocument = z.object({
	signatures: z
		.array(z.object({ keyId: z.string(), algorithm: z.literal('ed25519'), signature: z.string() }))
		.min(1)
		.max(4),
});
const Feed = z.object({ channel: z.string(), platform: z.string(), version: z.string() });

function releaseKeys(): string[] {
	const source = readFileSync(SIGNATURE_SOURCE, 'utf8');
	const table = /const RELEASE_KEYS: &\[TrustedKey\] = &\[([\s\S]*?)\];/u.exec(source)?.[1] ?? '';
	return [...table.matchAll(/TrustedKey::from_hex\(\s*"([0-9a-f]{64})"/gu)].flatMap((match) =>
		match[1] === undefined ? [] : [match[1]],
	);
}

function verifiesWithReleaseKey(document: Buffer, signature: Buffer): boolean {
	const signed = Buffer.concat([FEED_DOMAIN, document]);
	return SignatureDocument.parse(JSON.parse(signature.toString('utf8'))).signatures.some((entry) =>
		releaseKeys().some((publicHex) => {
			const raw = Buffer.from(publicHex, 'hex');
			if (createHash('sha256').update(raw).digest('hex').slice(0, 16) !== entry.keyId) return false;
			const key = createPublicKey({
				key: Buffer.concat([ED25519_SPKI, raw]),
				format: 'der',
				type: 'spki',
			});
			return verify(null, signed, key, Buffer.from(entry.signature, 'base64'));
		}),
	);
}

function trackedPairs(): { channel: string; platform: string; folder: URL }[] {
	if (!existsSync(UPDATE_DIRECTORY)) return [];
	return readdirSync(UPDATE_DIRECTORY).flatMap((channel) =>
		readdirSync(new URL(`${channel}/`, UPDATE_DIRECTORY)).map((platform) => ({
			channel,
			platform,
			folder: new URL(`${channel}/${platform}/`, UPDATE_DIRECTORY),
		})),
	);
}

describe('tracked update feed', () => {
	it('is signed by a release key and names the version the download button offers', () => {
		// Nothing is tracked before the first update-capable release.
		for (const { channel, platform, folder } of trackedPairs()) {
			expect(new Set(readdirSync(folder))).toEqual(new Set(['feed.json.sig', 'feed.json.signed']));
			const document = readFileSync(new URL('feed.json.signed', folder));
			const signature = readFileSync(new URL('feed.json.sig', folder));
			expect(verifiesWithReleaseKey(document, signature)).toBe(true);
			const feed = Feed.parse(JSON.parse(document.toString('utf8')));
			expect(feed.channel).toBe(channel);
			expect(feed.platform).toBe(platform);
			expect(release.published && release.version).toBe(feed.version);
		}
	});
});
