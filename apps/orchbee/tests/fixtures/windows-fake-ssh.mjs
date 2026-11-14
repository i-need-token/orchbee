// A stand-in for OpenSSH in the machine hub's process tests only; it is never shipped. Run with
// `-V` alone it prints the plan's release line, by default Windows OpenSSH 9.5's, on standard error
// as OpenSSH does, and records nothing. Otherwise it asks the plan's questions through SSH_ASKPASS in
// OpenSSH 9.5's words, one at a time, compares each answer by its SHA-256 digest so no test answer
// is written anywhere, records only what happened, and then runs the Client under test as the far
// end. It ends as OpenSSH does: exit 255 with OpenSSH's refusal on standard error when an answer is
// wrong too often, the askpass program fails, or the plan names a changed host key, which OpenSSH
// refuses without asking; a step marked `drop` ends after its first answer, right or wrong, the
// way a server that closes the connection after too many tries makes OpenSSH end; and a step with
// `vanish` ends that many milliseconds after it started its helper, the question still open, with
// OpenSSH's words for a connection the server closed.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFileSync, readFileSync, writeSync } from 'node:fs';

const plan = JSON.parse(readFileSync(process.env.FAKE_SSH_PLAN, 'utf8'));
const note = (line) => appendFileSync(process.env.FAKE_SSH_TRANSCRIPT, `${line}\n`);
const digest = (text) => createHash('sha256').update(text, 'utf8').digest('hex');
const refuse = (text) => {
	writeSync(2, text);
	process.exit(255);
};
const DENIED =
	'user@host.invalid: Permission denied (publickey,password,keyboard-interactive).\r\n';
const UNVERIFIED = 'Host key verification failed.\r\n';
const DISCONNECTED =
	'Received disconnect from 192.0.2.1 port 22:2: Too many authentication failures\r\n' +
	'Disconnected from 192.0.2.1 port 22\r\n';
const CLOSED = 'Connection closed by 192.0.2.1 port 22\r\n';

if (process.argv.length === 3 && process.argv[2] === '-V') {
	writeSync(2, `${plan.version ?? 'OpenSSH_for_Windows_9.5p1, LibreSSL 3.8.2'}\r\n`);
	process.exit(0);
}
note('started');
if (plan.changed) {
	note('changed');
	refuse(
		[
			'@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@',
			'@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @',
			'@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@',
			'IT IS POSSIBLE THAT SOMEONE IS DOING SOMETHING NASTY!',
			'The fingerprint for the ED25519 key sent by the remote host is',
			`${plan.changed}.`,
			'Host key verification failed.',
			'',
		].join('\r\n'),
	);
}
let asked = 0;
for (const step of plan.questions) {
	let tries = step.tries ?? 1;
	const refusal = step.host ? UNVERIFIED : DENIED;
	for (;;) {
		asked += 1;
		note(`asked ${asked}`);
		if (step.vanish) {
			spawn(process.env.SSH_ASKPASS, [step.prompt], { stdio: 'ignore', windowsHide: true });
			Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, step.vanish);
			note(`vanished ${asked}`);
			refuse(CLOSED);
		}
		const helper = spawnSync(process.env.SSH_ASKPASS, [step.prompt], {
			stdio: ['ignore', 'pipe', 'pipe'],
			windowsHide: true,
		});
		if (helper.stderr.length > 0) {
			note(`helper-stderr ${asked}`);
		}
		if (helper.status !== 0) {
			note(`unanswered ${asked}`);
			refuse(refusal);
		}
		const answer = helper.stdout.toString('utf8').replace(/\r?\n$/, '');
		helper.stdout.fill(0);
		const right = digest(answer) === step.accept;
		note(`${right ? 'right' : 'wrong'} ${asked}`);
		if (step.drop) {
			refuse(DISCONNECTED);
		}
		if (right) {
			break;
		}
		tries -= 1;
		if (tries === 0) {
			refuse(refusal);
		}
	}
}
note('linked');
const far = spawn(process.env.FAKE_SSH_CLIENT, process.argv.slice(2), {
	stdio: 'inherit',
	windowsHide: true,
});
far.on('exit', (code) => process.exit(code ?? 1));
