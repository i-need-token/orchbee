import { createHash } from 'node:crypto';

import {
	SshEnvironmentRuntimeInstanceIdSchema,
	type SshEnvironmentRuntimeInstanceId,
} from '@orchbee/ssh-environment-protocol';

import { SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES } from './ssh-environment-bootstrap-negotiation';
import type { VerifiedSshEnvironmentReleaseCatalog } from './verify-ssh-environment-release';

const REMOTE_RUNTIME_EXECUTABLE_NAME = 'orchbee-ssh-environment-runtime';
const REMOTE_TERMINAL_HOST_EXECUTABLE_NAME = 'orchbee-terminal-host';
const TERMINAL_HOST_ENDPOINT_DOMAIN = 'orchbee-terminal-host-endpoint-v1\0';
const RUNTIME_COHORT_NAMESPACE_HEX_LENGTH = 16;

export interface SshEnvironmentRemoteBootstrap {
	readonly command: string;
	readonly runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
	readonly terminalHostEndpointId: string;
}

export function createSshEnvironmentRemoteBootstrap(options: {
	readonly catalog: VerifiedSshEnvironmentReleaseCatalog;
	readonly runtimeInstanceId: string;
}): SshEnvironmentRemoteBootstrap {
	const runtimeInstanceId = SshEnvironmentRuntimeInstanceIdSchema.parse(options.runtimeInstanceId);
	const { catalog } = options;
	const runtime = catalog.components['ssh-environment-runtime'];
	const terminalHost = catalog.components['terminal-host'];
	const runtimeCohortNamespace = catalog.releaseCohortDigest.slice(
		0,
		RUNTIME_COHORT_NAMESPACE_HEX_LENGTH,
	);
	const terminalHostEndpointId = createTerminalHostEndpointId(
		runtimeInstanceId,
		catalog.releaseCohortDigest,
	);
	const darwinRuntime = runtime.artifacts['darwin-arm64'];
	const darwinTerminalHost = terminalHost.artifacts['darwin-arm64'];
	const linuxRuntime = runtime.artifacts['linux-x64'];
	const linuxTerminalHost = terminalHost.artifacts['linux-x64'];
	const command = [
		'set -eu',
		'umask 077',
		'case "$(uname -s):$(uname -m)" in',
		'  Darwin:arm64)',
		`    runtime_expected_digest=${darwinRuntime.buildDigest}`,
		`    runtime_expected_size=${darwinRuntime.sizeBytes}`,
		`    terminal_host_expected_digest=${darwinTerminalHost.buildDigest}`,
		`    terminal_host_expected_size=${darwinTerminalHost.sizeBytes}`,
		`    runtime_status_present='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'].present}'`,
		`    runtime_status_upload='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'].upload}'`,
		`    terminal_host_status_present='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['terminal-host'].present}'`,
		`    terminal_host_status_upload='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['terminal-host'].upload}'`,
		`    commit_status='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64'].committed}'`,
		'    ;;',
		'  Linux:x86_64)',
		'    command -v getconf >/dev/null 2>&1 || exit 71',
		'    getconf GNU_LIBC_VERSION >/dev/null 2>&1 || exit 71',
		`    runtime_expected_digest=${linuxRuntime.buildDigest}`,
		`    runtime_expected_size=${linuxRuntime.sizeBytes}`,
		`    terminal_host_expected_digest=${linuxTerminalHost.buildDigest}`,
		`    terminal_host_expected_size=${linuxTerminalHost.sizeBytes}`,
		`    runtime_status_present='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['linux-x64']['ssh-environment-runtime'].present}'`,
		`    runtime_status_upload='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['linux-x64']['ssh-environment-runtime'].upload}'`,
		`    terminal_host_status_present='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['linux-x64']['terminal-host'].present}'`,
		`    terminal_host_status_upload='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['linux-x64']['terminal-host'].upload}'`,
		`    commit_status='${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['linux-x64'].committed}'`,
		'    ;;',
		'  *) exit 71 ;;',
		'esac',
		'[ "${HOME#/}" != "$HOME" ] || exit 78',
		'orchbee_directory="$HOME/.orchbee"',
		'runtime_directory="$orchbee_directory/ssh-runtime"',
		'runtime_versions_directory="$runtime_directory/versions"',
		`runtime_version_directory="$runtime_versions_directory/${runtime.version}"`,
		'runtime_state_root_directory="$orchbee_directory/runtime-state"',
		`runtime_state_directory="$runtime_state_root_directory/${runtimeCohortNamespace}"`,
		`runtime_executable="$runtime_version_directory/${REMOTE_RUNTIME_EXECUTABLE_NAME}"`,
		'terminal_host_directory="$orchbee_directory/terminal-host"',
		'terminal_host_versions_directory="$terminal_host_directory/versions"',
		`terminal_host_version_directory="$terminal_host_versions_directory/${terminalHost.version}"`,
		'terminal_host_state_directory="$terminal_host_directory/state"',
		`terminal_host_executable="$terminal_host_version_directory/${REMOTE_TERMINAL_HOST_EXECUTABLE_NAME}"`,
		'ensure_private_directory() {',
		'  [ ! -L "$1" ] || exit 78',
		'  mkdir -p "$1"',
		'  [ -d "$1" ] && [ ! -L "$1" ] || exit 78',
		'  chmod 700 "$1"',
		'}',
		'ensure_private_directory "$orchbee_directory"',
		'ensure_private_directory "$runtime_directory"',
		'ensure_private_directory "$runtime_versions_directory"',
		'ensure_private_directory "$runtime_version_directory"',
		'ensure_private_directory "$runtime_state_root_directory"',
		'ensure_private_directory "$runtime_state_directory"',
		'ensure_private_directory "$terminal_host_directory"',
		'ensure_private_directory "$terminal_host_versions_directory"',
		'ensure_private_directory "$terminal_host_version_directory"',
		'ensure_private_directory "$terminal_host_state_directory"',
		'if command -v sha256sum >/dev/null 2>&1; then',
		'  hash_file() { sha256sum "$1" | awk "{print \\$1}"; }',
		'elif command -v shasum >/dev/null 2>&1; then',
		'  hash_file() { shasum -a 256 "$1" | awk "{print \\$1}"; }',
		'else',
		'  exit 71',
		'fi',
		'ensure_component() {',
		'  component_executable=$1',
		'  component_expected_digest=$2',
		'  component_expected_size=$3',
		'  component_status_present=$4',
		'  component_status_upload=$5',
		'  component_version_directory=$6',
		'  [ ! -L "$component_executable" ] || exit 78',
		'  if [ -e "$component_executable" ]; then',
		'    [ -f "$component_executable" ] || exit 78',
		'    installed_digest=$(hash_file "$component_executable")',
		'    [ "$installed_digest" = "$component_expected_digest" ] || exit 78',
		'    printf "%s\\n" "$component_status_present"',
		'    return',
		'  fi',
		'  printf "%s\\n" "$component_status_upload"',
		'  remaining=$component_expected_size',
		'  incoming="$component_version_directory/.incoming-$component_expected_digest-$$"',
		'  chunk="$component_version_directory/.chunk-$component_expected_digest-$$"',
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
		'  [ "$(wc -c < "$incoming" | tr -d " ")" -eq "$component_expected_size" ] || exit 74',
		'  [ "$(hash_file "$incoming")" = "$component_expected_digest" ] || exit 74',
		'  chmod 700 "$incoming"',
		'  if ln "$incoming" "$component_executable" 2>/dev/null; then',
		'    rm -f "$incoming"',
		'  else',
		'    [ ! -L "$component_executable" ] || exit 78',
		'    [ -f "$component_executable" ] || exit 78',
		'    installed_digest=$(hash_file "$component_executable")',
		'    [ "$installed_digest" = "$component_expected_digest" ] || exit 78',
		'  fi',
		'  rm -f "$chunk"',
		'  trap - EXIT HUP INT TERM',
		'}',
		'ensure_component "$runtime_executable" "$runtime_expected_digest" "$runtime_expected_size" "$runtime_status_present" "$runtime_status_upload" "$runtime_version_directory"',
		'ensure_component "$terminal_host_executable" "$terminal_host_expected_digest" "$terminal_host_expected_size" "$terminal_host_status_present" "$terminal_host_status_upload" "$terminal_host_version_directory"',
		'printf "%s\\n" "$commit_status"',
		'nohup "$runtime_executable" serve' +
			' --runtime-directory "$runtime_state_directory"' +
			` --instance-id '${runtimeInstanceId}'` +
			' --build-digest "$runtime_expected_digest"' +
			` --release-cohort-digest '${catalog.releaseCohortDigest}'` +
			` --terminal-host-version '${terminalHost.version}'` +
			' --terminal-host-build-digest "$terminal_host_expected_digest"' +
			' --terminal-host-executable "$terminal_host_executable"' +
			' --terminal-host-runtime-directory "$terminal_host_state_directory"' +
			` --terminal-host-endpoint-id '${terminalHostEndpointId}'` +
			' </dev/null >/dev/null 2>&1 &',
		'attempt=0',
		`while [ ! -S "$runtime_state_directory/${runtimeInstanceId}.s" ]; do`,
		'  attempt=$((attempt + 1))',
		'  [ "$attempt" -le 100 ] || exit 75',
		'  sleep 0.05',
		'done',
		'exec "$runtime_executable" attach --runtime-directory "$runtime_state_directory" --instance-id ' +
			`'${runtimeInstanceId}'`,
	].join('\n');

	return { command, runtimeInstanceId, terminalHostEndpointId };
}

function createTerminalHostEndpointId(
	runtimeInstanceId: SshEnvironmentRuntimeInstanceId,
	releaseCohortDigest: string,
): string {
	return createHash('sha256')
		.update(TERMINAL_HOST_ENDPOINT_DOMAIN)
		.update(runtimeInstanceId)
		.update('\0')
		.update(releaseCohortDigest)
		.digest('hex')
		.slice(0, 32);
}
