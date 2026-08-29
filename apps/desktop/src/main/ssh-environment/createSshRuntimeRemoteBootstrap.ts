import {
	SshEnvironmentRuntimeInstanceIdSchema,
	type SshEnvironmentRuntimeInstanceId,
} from '@orchbee/ssh-environment-protocol';

import { SSH_RUNTIME_BOOTSTRAP_STATUS_LINES } from './ssh-runtime-bootstrap-negotiation';
import type { VerifiedSshRuntimeReleaseCatalog } from './verify-ssh-runtime-artifact';

const REMOTE_RUNTIME_EXECUTABLE_NAME = 'orchbee-ssh-environment-runtime';

export interface SshRuntimeRemoteBootstrap {
	readonly command: string;
	readonly runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
}

export function createSshRuntimeRemoteBootstrap(options: {
	readonly catalog: VerifiedSshRuntimeReleaseCatalog;
	readonly runtimeInstanceId: string;
}): SshRuntimeRemoteBootstrap {
	const runtimeInstanceId = SshEnvironmentRuntimeInstanceIdSchema.parse(options.runtimeInstanceId);
	const { catalog } = options;
	const darwin = catalog.artifacts['darwin-arm64'];
	const linux = catalog.artifacts['linux-x64'];
	const command = [
		'set -eu',
		'umask 077',
		'case "$(uname -s):$(uname -m)" in',
		'  Darwin:arm64)',
		`    expected_digest=${darwin.buildDigest}`,
		`    expected_size=${darwin.sizeBytes}`,
		`    status_present='${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].present}'`,
		`    status_upload='${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].upload}'`,
		'    ;;',
		'  Linux:x86_64)',
		'    command -v getconf >/dev/null 2>&1 || exit 71',
		'    getconf GNU_LIBC_VERSION >/dev/null 2>&1 || exit 71',
		`    expected_digest=${linux.buildDigest}`,
		`    expected_size=${linux.sizeBytes}`,
		`    status_present='${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['linux-x64'].present}'`,
		`    status_upload='${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['linux-x64'].upload}'`,
		'    ;;',
		'  *) exit 71 ;;',
		'esac',
		'orchbee_directory="$HOME/.orchbee"',
		'runtime_directory="$orchbee_directory/ssh-runtime"',
		'versions_directory="$runtime_directory/versions"',
		`version_directory="$versions_directory/${catalog.runtimeVersion}"`,
		'state_directory="$runtime_directory/state"',
		`runtime_executable="$version_directory/${REMOTE_RUNTIME_EXECUTABLE_NAME}"`,
		'ensure_private_directory() {',
		'  [ ! -L "$1" ] || exit 78',
		'  mkdir -p "$1"',
		'  [ -d "$1" ] && [ ! -L "$1" ] || exit 78',
		'  chmod 700 "$1"',
		'}',
		'ensure_private_directory "$orchbee_directory"',
		'ensure_private_directory "$runtime_directory"',
		'ensure_private_directory "$versions_directory"',
		'ensure_private_directory "$version_directory"',
		'ensure_private_directory "$state_directory"',
		'if command -v sha256sum >/dev/null 2>&1; then',
		'  hash_file() { sha256sum "$1" | awk "{print \\$1}"; }',
		'elif command -v shasum >/dev/null 2>&1; then',
		'  hash_file() { shasum -a 256 "$1" | awk "{print \\$1}"; }',
		'else',
		'  exit 71',
		'fi',
		'[ ! -L "$runtime_executable" ] || exit 78',
		'if [ -e "$runtime_executable" ]; then',
		'  [ -f "$runtime_executable" ] || exit 78',
		'  installed_digest=$(hash_file "$runtime_executable")',
		'  [ "$installed_digest" = "$expected_digest" ] || exit 78',
		'  printf "%s\\n" "$status_present"',
		'else',
		'  printf "%s\\n" "$status_upload"',
		'  remaining=$expected_size',
		'  incoming="$version_directory/.incoming-$expected_digest-$$"',
		'  chunk="$version_directory/.chunk-$expected_digest-$$"',
		'  cleanup() { rm -f "$incoming" "$chunk"; }',
		'  trap cleanup EXIT HUP INT TERM',
		'  rm -f "$incoming" "$chunk"',
		'  : > "$incoming"',
		'  while [ "$remaining" -gt 0 ]; do',
		'    block=$remaining',
		'    [ "$block" -le 1048576 ] || block=1048576',
		'    rm -f "$chunk"',
		'    dd bs="$block" count=1 of="$chunk" 2>/dev/null || exit 74',
		'    received=$(wc -c < "$chunk" | tr -d " ")',
		'    [ "$received" -gt 0 ] || exit 74',
		'    cat "$chunk" >> "$incoming"',
		'    remaining=$((remaining - received))',
		'  done',
		'  [ "$(wc -c < "$incoming" | tr -d " ")" -eq "$expected_size" ] || exit 74',
		'  [ "$(hash_file "$incoming")" = "$expected_digest" ] || exit 74',
		'  chmod 700 "$incoming"',
		'  if ln "$incoming" "$runtime_executable" 2>/dev/null; then',
		'    rm -f "$incoming"',
		'  else',
		'    [ ! -L "$runtime_executable" ] || exit 78',
		'    [ -f "$runtime_executable" ] || exit 78',
		'    installed_digest=$(hash_file "$runtime_executable")',
		'    [ "$installed_digest" = "$expected_digest" ] || exit 78',
		'  fi',
		'  rm -f "$chunk"',
		'  trap - EXIT HUP INT TERM',
		'fi',
		'nohup "$runtime_executable" serve --runtime-directory "$state_directory" --instance-id ' +
			runtimeInstanceId +
			' --build-digest "$expected_digest"' +
			' </dev/null >/dev/null 2>&1 &',
		'attempt=0',
		`while [ ! -S "$state_directory/${runtimeInstanceId}.s" ]; do`,
		'  attempt=$((attempt + 1))',
		'  [ "$attempt" -le 100 ] || exit 75',
		'  sleep 0.05',
		'done',
		'exec "$runtime_executable" attach --runtime-directory "$state_directory" --instance-id ' +
			runtimeInstanceId,
	].join('\n');

	return { command, runtimeInstanceId };
}
