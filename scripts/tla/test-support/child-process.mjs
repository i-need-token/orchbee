import { execFile } from 'node:child_process';

// Isolated CLI fixtures may overlap while each invocation still waits for stdio closure.
export function runChild(command, args, options) {
	return new Promise((resolve) => {
		execFile(command, args, { maxBuffer: 4 * 1024 ** 2, ...options }, (error, stdout, stderr) => {
			resolve({
				status: error ? (typeof error.code === 'number' ? error.code : null) : 0,
				signal: error?.signal ?? null,
				error: error && typeof error.code !== 'number' ? error : undefined,
				stdout,
				stderr,
			});
		});
	});
}
