import { sanitizeAgentHookInput } from './hook-sanitizers.mjs';
import { forwardSanitizedObservation } from './observation-transport.mjs';

const MAX_INPUT_BYTES = 64 * 1024;

await main().catch(() => undefined);

async function main() {
	const input = await readInput();
	if (input === null) return;
	let raw;
	try {
		raw = JSON.parse(input);
	} catch {
		return;
	}
	const adapterId = process.argv[2];
	const observation = sanitizeAgentHookInput(adapterId, raw);
	if (observation === null) return;
	await forwardSanitizedObservation(adapterId, observation);
}

async function readInput() {
	const chunks = [];
	let byteLength = 0;
	for await (const chunk of process.stdin) {
		const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
		byteLength += bytes.byteLength;
		if (byteLength > MAX_INPUT_BYTES) return null;
		chunks.push(bytes);
	}
	return Buffer.concat(chunks, byteLength).toString('utf8');
}
