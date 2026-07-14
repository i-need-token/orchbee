import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { Terminal, type TerminalConfig } from '../composition';
import { init, type WasmBridge } from '../wasm-bridge';

let bridgePromise: Promise<WasmBridge> | undefined;

export function initializeWasm(): Promise<WasmBridge> {
	if (!bridgePromise) {
		const wasmPath = fileURLToPath(new URL('../../ghostty-vt.wasm', import.meta.url));
		const bytes = readFileSync(wasmPath);
		const source = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
		bridgePromise = init({ source });
	}
	return bridgePromise;
}

export function createTerminal(cols = 80, rows = 24, config?: TerminalConfig): Terminal {
	return new Terminal(cols, rows, config);
}
