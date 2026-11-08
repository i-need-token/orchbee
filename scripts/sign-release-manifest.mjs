// Sign a packaged release manifest for `orchbee update`: write the detached signature document
// `release.json.sig` beside `release.json`. Each Ed25519 signature covers UTF-8
// `orchbee-release-manifest-v2`, LF, and the exact manifest bytes. Keys are PKCS #8 PEM from files
// or environment variables; the release workflow passes `--key-env ORCHBEE_RELEASE_SIGNING_KEY`.
// No upload or network access occurs here, and no key material is ever printed.
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

const domain = Buffer.from('orchbee-release-manifest-v2\n', 'utf8');
// The updater reads at most these many bytes of each document; these bounds never shrink.
const maximumManifest = 1024 * 1024;
const maximumDocument = 4 * 1024;
const maximumSignatures = 4;
const usage =
	'Usage: node scripts/sign-release-manifest.mjs <absolute release.json> (--key-file <absolute PEM> | --key-env <VARIABLE>)...';

/** The first 16 hexadecimal digits of SHA-256 over the raw 32-byte Ed25519 public key. */
function keyId(privateKey) {
	const spki = createPublicKey(privateKey).export({ format: 'der', type: 'spki' });
	// An Ed25519 SubjectPublicKeyInfo ends with the raw 32-byte public key.
	return createHash('sha256')
		.update(spki.subarray(spki.length - 32))
		.digest('hex')
		.slice(0, 16);
}

/** Admits one PKCS #8 PEM private key, refusing every algorithm except Ed25519. */
export function loadKey(pem) {
	const key = createPrivateKey({ key: pem, format: 'pem' });
	if (key.asymmetricKeyType !== 'ed25519') throw new Error('Signing keys must be Ed25519');
	return key;
}

/** The signature document over `manifest` with one entry per distinct key, 1 to 4 keys. */
export function signatureDocument(manifest, keys) {
	if (!Buffer.isBuffer(manifest) || manifest.length > maximumManifest)
		throw new Error('Release manifest exceeds 1 MiB');
	if (keys.length === 0 || keys.length > maximumSignatures)
		throw new Error('Sign with 1 to 4 keys');
	const signed = Buffer.concat([domain, manifest]);
	const signatures = keys.map((key) => {
		const signature = sign(null, signed, key);
		// Never publish a signature that its own public key does not verify.
		if (!verify(null, signed, createPublicKey(key), signature))
			throw new Error('Signature self-check failed');
		return { keyId: keyId(key), algorithm: 'ed25519', signature: signature.toString('base64') };
	});
	if (new Set(signatures.map((entry) => entry.keyId)).size !== signatures.length)
		throw new Error('Signing keys must be distinct');
	const document = JSON.stringify({ signatures }) + '\n';
	if (Buffer.byteLength(document) > maximumDocument)
		throw new Error('Signature document exceeds 4 KiB');
	return document;
}

/** Signs an existing `release.json` and creates `release.json.sig`, never replacing one. */
export function signManifest(manifestPath, keys) {
	if (!path.isAbsolute(manifestPath) || path.basename(manifestPath) !== 'release.json')
		throw new Error('Expected an absolute path to release.json');
	const metadata = lstatSync(manifestPath);
	if (!metadata.isFile() || metadata.size > maximumManifest)
		throw new Error('Release manifest must be a regular file of at most 1 MiB');
	const manifest = readFileSync(manifestPath);
	// A signature must never bless a file the updater cannot read as a version 2 manifest.
	const parsed = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(manifest));
	if (parsed?.schemaVersion !== 2 || parsed.product !== 'orchbee')
		throw new Error('Expected an orchbee release manifest with schemaVersion 2');
	const document = signatureDocument(manifest, keys);
	const destination = manifestPath + '.sig';
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
	return destination;
}

/** The manifest path and PEM keys named by `argv`, reading environment keys from `environment`. */
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
	const { manifest, keys } = parseArguments(process.argv.slice(2), process.env);
	process.stdout.write(signManifest(manifest, keys) + '\n');
}
