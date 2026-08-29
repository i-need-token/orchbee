import { SshAskPassHelperClient } from './SshAskPassHelperClient';

const client = new SshAskPassHelperClient();
void client.run().then((exitCode) => {
	process.exitCode = exitCode;
});
