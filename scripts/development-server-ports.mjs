import { realpathSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { developmentArtifactStem } from './stage-local-runtime.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
// Checkouts share 4000 slots of two adjacent ports, 41000–48999, below the dynamic range at 49152.
const FIRST_PORT = 41_000;
const SLOTS = 4_000;

/**
 * The loopback ports a checkout's development servers listen on: the Web renderer on the slot's even
 * port and the Desktop Main and preload server on the odd one after it. The slot comes from the
 * checkout digest that names its development cohort, so two checkouts run `pnpm desktop` side by
 * side; packaged builds run no development server.
 */
export function developmentServerPorts(sourceRoot = root, platform = process.platform) {
	const stem = developmentArtifactStem(realpathSync.native(sourceRoot), platform);
	const slot = Number.parseInt(stem.slice('ob-dev-'.length, 'ob-dev-'.length + 8), 16) % SLOTS;
	const web = FIRST_PORT + 2 * slot;
	return { desktop: web + 1, web };
}
