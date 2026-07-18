import {
	checkStyleContract,
	formatStyleDiagnostics,
	repositoryRootFrom,
} from './style-contract.mjs';

const repositoryRoot = repositoryRootFrom(import.meta.dirname);
const diagnostics = await checkStyleContract(repositoryRoot);

if (diagnostics.length > 0) {
	process.stderr.write(`${formatStyleDiagnostics(diagnostics)}\n`);
	process.exitCode = 1;
} else {
	process.stdout.write('Verified the frontend style contract\n');
}
