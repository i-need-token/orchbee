/**
 * Whether `candidate` takes precedence over `than` as Semantic Versioning orders versions; build
 * metadata is ignored, and a version either side cannot read is never newer.
 */
export function isNewerProductVersion(candidate: string, than: string): boolean {
	const left = parsePrecedence(candidate);
	const right = parsePrecedence(than);
	return left !== null && right !== null && comparePrecedence(left, right) > 0;
}

/**
 * A total order of product versions: Semantic Versioning precedence, a readable version above one
 * that cannot be read, and two unreadable versions in code unit order.
 */
// Cross-runtime contract: search for `upgrade-backup-folders` before changing this order; native
// writers keep the same upgrade folders.
export function compareProductVersions(left: string, right: string): number {
	const leftPrecedence = parsePrecedence(left);
	const rightPrecedence = parsePrecedence(right);
	if (leftPrecedence !== null && rightPrecedence !== null) {
		return comparePrecedence(leftPrecedence, rightPrecedence);
	}
	if (leftPrecedence !== null) return 1;
	if (rightPrecedence !== null) return -1;
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

interface Precedence {
	readonly core: readonly [number, number, number];
	readonly preRelease: readonly string[];
}

function parsePrecedence(text: string): Precedence | null {
	const [version = ''] = text.split('+', 1);
	const separator = version.indexOf('-');
	const core = separator === -1 ? version : version.slice(0, separator);
	const preRelease = separator === -1 ? [] : version.slice(separator + 1).split('.');
	const numbers = core.split('.');
	if (numbers.length !== 3 || numbers.some((part) => !/^\d+$/u.test(part))) return null;
	if (preRelease.some((part) => part.length === 0)) return null;
	const [major = 0, minor = 0, patch = 0] = numbers.map(Number);
	return { core: [major, minor, patch], preRelease };
}

function comparePrecedence(left: Precedence, right: Precedence): number {
	for (let index = 0; index < 3; index += 1) {
		const order = (left.core[index] ?? 0) - (right.core[index] ?? 0);
		if (order !== 0) return Math.sign(order);
	}
	if (left.preRelease.length === 0 || right.preRelease.length === 0) {
		return Math.sign(right.preRelease.length - left.preRelease.length) || 0;
	}
	const shared = Math.min(left.preRelease.length, right.preRelease.length);
	for (let index = 0; index < shared; index += 1) {
		const order = compareIdentifier(left.preRelease[index] ?? '', right.preRelease[index] ?? '');
		if (order !== 0) return order;
	}
	return Math.sign(left.preRelease.length - right.preRelease.length);
}

function compareIdentifier(left: string, right: string): number {
	const leftNumeric = /^\d+$/u.test(left);
	const rightNumeric = /^\d+$/u.test(right);
	if (leftNumeric && rightNumeric) return Math.sign(Number(left) - Number(right));
	if (leftNumeric) return -1;
	if (rightNumeric) return 1;
	if (left === right) return 0;
	return left < right ? -1 : 1;
}
