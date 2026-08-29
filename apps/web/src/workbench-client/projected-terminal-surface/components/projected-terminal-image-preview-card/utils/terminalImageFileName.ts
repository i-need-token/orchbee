export function terminalImageFileName(filePath: string): string {
	return filePath.replaceAll('\\', '/').split('/').filter(Boolean).at(-1) ?? filePath;
}
