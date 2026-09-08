import { spawn } from 'node:child_process';

const targets = ['x86_64-linux-gnu', 'x86_64-freebsd', 'aarch64-macos', 'x86_64-windows-gnu'];

const optimize = 'ReleaseSafe';
const jobs = targets.map((target) => {
	const child = spawn(
		'zig',
		['build', 'test-compile', `-Dtarget=${target}`, `-Doptimize=${optimize}`],
		{
			stdio: ['ignore', 'inherit', 'pipe'],
		},
	);
	const label = target;
	child.stderr.on('data', (chunk) => {
		process.stderr.write(`[${label}] ${chunk}`);
	});
	return { label, child, code: null };
});

const exits = await Promise.all(
	jobs.map(
		({ label, child }) =>
			new Promise((resolve) => {
				child.on('exit', (code) => resolve({ label, code }));
				child.on('error', (error) => {
					console.error(`[${label}] ${error.message}`);
					resolve({ label, code: 127 });
				});
			}),
	),
);

const failures = exits.filter(({ code }) => code !== 0);
if (failures.length > 0) {
	console.error(`test:cross failed for: ${failures.map(({ label }) => label).join(', ')}`);
	process.exit(1);
}
