# Orchbee Bash integration. Source this file explicitly from a Bash startup file.

if [[ -n ${__ORCHBEE_SHELL_INTEGRATION_LOADED-} ]]; then
	return 0
fi
if (( BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 4) )); then
	ORCHBEE_SHELL_INTEGRATION_STATUS=unsupported-bash-version
	return 0
fi
__ORCHBEE_SHELL_INTEGRATION_LOADED=1

__orchbee_encode_path() {
	local LC_ALL=C input=$1 output='' character encoded index
	for ((index = 0; index < ${#input}; index += 1)); do
		character=${input:index:1}
		case $character in
			[a-zA-Z0-9/._~-]) output+=$character ;;
			*)
				printf -v encoded '%%%02X' "'$character"
				output+=$encoded
				;;
		esac
	done
	REPLY=$output
}

__orchbee_prompt_command() {
	local exit_code=$?
	printf '\033]133;D;%d\007' "$exit_code"
	__orchbee_encode_path "$PWD"
	printf '\033]7;file://%s\007' "$REPLY"
	printf '\033]133;A\007'
}

PS0=$'\033]133;C\007'${PS0-}
case $(declare -p PROMPT_COMMAND 2>/dev/null) in
	'declare -a'*) PROMPT_COMMAND=(__orchbee_prompt_command "${PROMPT_COMMAND[@]}") ;;
	*) PROMPT_COMMAND="__orchbee_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}" ;;
esac

__orchbee_ssh_observation_eligible() {
	[[ -n ${ORCHBEE_AGENT_OBSERVATION_SOCKET-} && -S ${ORCHBEE_AGENT_OBSERVATION_SOCKET-} && -t 0 && -t 1 ]] || return 1
	local argument
	for argument in "$@"; do
		case $argument in
			-G|-O|-Q|-V|-W|-W*) return 1 ;;
		esac
		if [[ $argument =~ ^-[46AaCfGgKkMNnqsTtVvXxYy]+$ && $argument =~ [NnfT] ]]; then
			return 1
		fi
	done
}

__orchbee_ssh_observation_nonce() {
	local nonce
	nonce=$(od -An -N16 -tx1 /dev/urandom 2>/dev/null | tr -d ' \n')
	if [[ ! $nonce =~ ^[a-f0-9]{32}$ ]]; then
		nonce="${BASHPID:-$$}${RANDOM:-0}${RANDOM:-0}$(date +%s 2>/dev/null)"
		nonce=${nonce//[^A-Za-z0-9_-]/}
	fi
	REPLY=$nonce
}

if [[ $(type -t ssh 2>/dev/null) == file ]]; then
	ssh() {
		if ! __orchbee_ssh_observation_eligible "$@"; then
			command ssh "$@"
			return $?
		fi
		__orchbee_ssh_observation_nonce
		local remote_socket="/tmp/orchbee-agent-v2-${REPLY}.sock"
		command ssh \
			-o StreamLocalBindMask=0177 \
			-o StreamLocalBindUnlink=yes \
			-R "${remote_socket}:${ORCHBEE_AGENT_OBSERVATION_SOCKET}" \
			"$@"
	}
fi
