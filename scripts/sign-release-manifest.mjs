// Sign a document `orchbee update` verifies, and verify such a signature. Two documents are signed:
// the packaged release manifest `release.json`, and the update feed document
// `<stem>-<version>-<platform>.feed.json` (scripts/release-feed.mjs). Each gets a detached signature
// document beside it, the same name plus `.sig`. Each Ed25519 signature covers a UTF-8 domain, LF,
// and the exact document bytes: `orchbee-release-manifest-v2` for a manifest and
// `orchbee-update-feed-v1` for a feed, so a signature of one never verifies as the other. Keys are
// PKCS #8 PEM from files or environment variables; the release workflow passes
// `--key-env ORCHBEE_RELEASE_SIGNING_KEY`. No upload or network access occurs here, and no key
// material is ever printed. Node built-ins only.
import { createHash, createPrivateKey, createPublicKey, sign, verify } from 'node:crypto';
import {
	closeSync,
	fsyncSync,
	lstatSync,
	openSync,
	readFileSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { FEED_LIMIT, readFeed, writtenForm } from './release-feed.mjs';

/** What one kind of signed document is signed under, and the reader's bound on its size. */
export const MANIFEST_DOMAIN = Object.freeze({
	prefix: 'orchbee-release-manifest-v2',
	limit: 1024 * 1024,
	description: 'Release manifest exceeds 1 MiB',
});
export const FEED_DOMAIN = Object.freeze({
	prefix: 'orchbee-update-feed-v1',
	limit: FEED_LIMIT,
	description: 'Update feed exceeds 64 KiB',
});
// The updater reads at most these many bytes of a signature document; this bound never shrinks.
const maximumDocument = 4 * 1024;
const maximumSignatures = 4;
// DER SubjectPublicKeyInfo header of an Ed25519 key; the raw 32-byte key follows it.
const ed25519Spki = Buffer.from('302a300506032b6570032100', 'hex');
const usage =
	'Usage: node scripts/sign-release-manifest.mjs <absolute release.json or <stem>-<version>-<platform>.feed.json> (--key-file <absolute PEM> | --key-env <VARIABLE>)...';

function signedBytes(document, domain) {
	return Buffer.concat([Buffer.from(`${domain.prefix}\n`, 'utf8'), document]);
}

/** The first 16 hexadecimal digits of SHA-256 over a raw 32-byte Ed25519 public key. */
function rawKeyId(raw) {
	return createHash('sha256').update(raw).digest('hex').slice(0, 16);
}

function keyId(privateKey) {
	const spki = createPublicKey(privateKey).export({ format: 'der', type: 'spki' });
	// An Ed25519 SubjectPublicKeyInfo ends with the raw 32-byte public key.
	return rawKeyId(spki.subarray(spki.length - 32));
}

/** Admits one PKCS #8 PEM private key, refusing every algorithm except Ed25519. */
export function loadKey(pem) {
	const key = createPrivateKey({ key: pem, format: 'pem' });
	if (key.asymmetricKeyType !== 'ed25519') throw new Error('Signing keys must be Ed25519');
	return key;
}

/** The signature document over `document` under `domain`, one entry per distinct key, 1 to 4 keys. */
export function signatureDocument(document, keys, domain = MANIFEST_DOMAIN) {
	if (!Buffer.isBuffer(document) || document.length > domain.limit)
		throw new Error(domain.description);
	if (keys.length === 0 || keys.length > maximumSignatures)
		throw new Error('Sign with 1 to 4 keys');
	const signed = signedBytes(document, domain);
	const signatures = keys.map((key) => {
		const signature = sign(null, signed, key);
		// Never publish a signature that its own public key does not verify.
		if (!verify(null, signed, createPublicKey(key), signature))
			throw new Error('Signature self-check failed');
		return { keyId: keyId(key), algorithm: 'ed25519', signature: signature.toString('base64') };
	});
	if (new Set(signatures.map((entry) => entry.keyId)).size !== signatures.length)
		throw new Error('Signing keys must be distinct');
	const text = JSON.stringify({ signatures }) + '\n';
	if (Buffer.byteLength(text) > maximumDocument)
		throw new Error('Signature document exceeds 4 KiB');
	return text;
}

/** The `(keyId, signature)` entries of a closed signature document, or undefined when malformed. */
function signatureEntries(bytes) {
	if (bytes.length > maximumDocument) return undefined;
	let root;
	try {
		root = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
	} catch {
		return undefined;
	}
	const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);
	if (!isObject(root) || !Array.isArray(root.signatures)) return undefined;
	if (Object.keys(root).some((name) => name !== 'signatures' && name !== 'extensions'))
		return undefined;
	if (root.extensions !== undefined && !isObject(root.extensions)) return undefined;
	if (root.signatures.length === 0 || root.signatures.length > maximumSignatures) return undefined;
	const entries = [];
	for (const entry of root.signatures) {
		if (
			!isObject(entry) ||
			Object.keys(entry).toSorted().join() !== 'algorithm,keyId,signature' ||
			entry.algorithm !== 'ed25519' ||
			typeof entry.keyId !== 'string' ||
			!/^[0-9a-f]{16}$/u.test(entry.keyId) ||
			typeof entry.signature !== 'string'
		)
			return undefined;
		const signature = Buffer.from(entry.signature, 'base64');
		// Canonical standard base64 of exactly 64 bytes, as the updater requires.
		if (signature.length !== 64 || signature.toString('base64') !== entry.signature)
			return undefined;
		entries.push({ keyId: entry.keyId, signature });
	}
	return entries;
}

/**
 * Whether `signature` (a signature document) admits `document` under `domain` with one of the
 * 64-hex public keys of `table`, as the updater decides: `trusted` with the signing key's id, or
 * `no-keys`, `too-large`, `malformed`, `untrusted` (no entry names a table key) or `invalid` (one
 * does, but none verifies).
 */
export function verifySignature(document, signature, table, domain) {
	if (table.length === 0) return { outcome: 'no-keys' };
	if (document.length > domain.limit) return { outcome: 'too-large' };
	const entries = signatureEntries(signature);
	if (entries === undefined) return { outcome: 'malformed' };
	const signed = signedBytes(document, domain);
	let named = false;
	for (const entry of entries) {
		for (const publicHex of table) {
			const raw = Buffer.from(publicHex, 'hex');
			if (rawKeyId(raw) !== entry.keyId) continue;
			named = true;
			const key = createPublicKey({
				key: Buffer.concat([ed25519Spki, raw]),
				format: 'der',
				type: 'spki',
			});
			if (verify(null, signed, key, entry.signature))
				return { outcome: 'trusted', keyId: entry.keyId };
		}
	}
	return { outcome: named ? 'invalid' : 'untrusted' };
}

/**
 * The 64-hex public keys of the table `name` (`RELEASE_KEYS` or `TEST_KEYS`) in the Rust source
 * apps/orchbee/src/cli/update/signature.rs, or undefined when the table is not found.
 */
export function keyTable(source, name) {
	const table = new RegExp(`const ${name}: &\\[TrustedKey\\] = &\\[([\\s\\S]*?)\\];`, 'u').exec(
		source,
	);
	if (table === null) return undefined;
	const body = table[1].replace(/\/\/[^\n]*/gu, '');
	return [...body.matchAll(/TrustedKey::from_hex\(\s*"([0-9a-f]{64})"\s*,?\s*\)/gu)].map(
		(match) => match[1],
	);
}

/** Writes `document` to `destination`, a new file, flushed. */
function createFile(destination, document) {
	const fd = openSync(destination, 'wx');
	try {
		writeFileSync(fd, document);
		fsyncSync(fd);
	} catch (error) {
		closeSync(fd);
		unlinkSync(destination);
		throw error;
	}
	closeSync(fd);
}

function readRegular(filename, limit, what) {
	const metadata = lstatSync(filename);
	if (!metadata.isFile() || metadata.size > limit)
		throw new Error(`${what} must be a regular file of at most ${limit} bytes`);
	return readFileSync(filename);
}

/** Signs an existing `release.json` and creates `release.json.sig`, never replacing one. */
export function signManifest(manifestPath, keys) {
	if (!path.isAbsolute(manifestPath) || path.basename(manifestPath) !== 'release.json')
		throw new Error('Expected an absolute path to release.json');
	const manifest = readRegular(manifestPath, MANIFEST_DOMAIN.limit, 'Release manifest');
	// A signature must never bless a file the updater cannot read as a version 2 manifest.
	const parsed = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(manifest));
	if (parsed?.schemaVersion !== 2 || parsed.product !== 'orchbee')
		throw new Error('Expected an orchbee release manifest with schemaVersion 2');
	const destination = manifestPath + '.sig';
	createFile(destination, signatureDocument(manifest, keys));
	return destination;
}

/** Signs an existing feed document and creates `<name>.sig` beside it, never replacing one. */
export function signFeed(feedPath, keys) {
	if (!path.isAbsolute(feedPath) || !path.basename(feedPath).endsWith('.feed.json'))
		throw new Error('Expected an absolute path to a <stem>-<version>-<platform>.feed.json');
	const feed = readRegular(feedPath, FEED_LIMIT, 'Update feed');
	// A signature must never bless a document the updater refuses. The written form also excludes
	// what JSON.parse admits and the updater's reader does not: a byte-order mark, a repeated
	// member, `1.0` for a number.
	const { feed: parsed, problems } = readFeed(feed);
	if (problems.length > 0) throw new Error(problems.join('\n'));
	if (!writtenForm(feed, parsed))
		throw new Error('The feed document is not in its written form; write it with release-feed.mjs');
	const destination = feedPath + '.sig';
	createFile(destination, signatureDocument(feed, keys, FEED_DOMAIN));
	return destination;
}

/** The document path and PEM keys named by `argv`, reading environment keys from `environment`. */
export function parseArguments(argv, environment) {
	const [manifest, ...options] = argv;
	if (manifest === undefined || options.length === 0 || options.length % 2 !== 0)
		throw new Error(usage);
	const keys = [];
	for (let index = 0; index < options.length; index += 2) {
		const [option, value] = [options[index], options[index + 1]];
		if (option === '--key-file' && path.isAbsolute(value)) {
			keys.push(loadKey(readFileSync(value)));
		} else if (option === '--key-env' && /^[A-Z][A-Z0-9_]*$/u.test(value)) {
			const pem = environment[value];
			if (pem === undefined || pem.trim() === '') throw new Error(`${value} holds no key`);
			keys.push(loadKey(pem));
		} else {
			throw new Error(usage);
		}
	}
	return { manifest, keys };
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const { manifest: document, keys } = parseArguments(process.argv.slice(2), process.env);
	const destination = document.endsWith('.feed.json')
		? signFeed(document, keys)
		: signManifest(document, keys);
	process.stdout.write(destination + '\n');
}
