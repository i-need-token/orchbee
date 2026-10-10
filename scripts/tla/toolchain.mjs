import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function provisionJar(tool, jarPath, allowDownload) {
	const matches = async (file) => {
		try {
			return (
				createHash('sha256')
					.update(await readFile(file))
					.digest('hex') === tool.sha256
			);
		} catch (error) {
			if (error.code === 'ENOENT') return false;
			throw error;
		}
	};
	if (await matches(jarPath)) return;
	if (!allowDownload)
		throw new Error(`ORCHBEE_TLA2TOOLS_JAR does not match pinned TLA+ ${tool.version} SHA-256`);
	await mkdir(path.dirname(jarPath), { recursive: true });
	const response = await fetch(tool.downloadUrl);
	if (!response.ok)
		throw new Error(`Could not download ${tool.downloadUrl}: HTTP ${response.status}`);
	const temporaryPath = `${jarPath}.${randomUUID()}.download`;
	try {
		await writeFile(temporaryPath, new Uint8Array(await response.arrayBuffer()));
		if (!(await matches(temporaryPath)))
			throw new Error(`Downloaded TLA+ ${tool.version} jar failed its pinned SHA-256 check`);
		try {
			await rename(temporaryPath, jarPath);
		} catch (error) {
			// A concurrent provisioner may publish and open the same pinned jar on Windows.
			if (!(await matches(jarPath))) throw error;
		}
	} finally {
		await rm(temporaryPath, { force: true });
	}
}
