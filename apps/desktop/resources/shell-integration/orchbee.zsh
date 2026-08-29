# Orchbee Zsh integration. Source this file explicitly from a Zsh startup file.

if [[ -n ${__ORCHBEE_SHELL_INTEGRATION_LOADED-} ]]; then
	return 0
fi
typeset -g __ORCHBEE_SHELL_INTEGRATION_LOADED=1
typeset -g __ORCHBEE_COMMAND_ACTIVE=0

autoload -Uz add-zsh-hook

function __orchbee_encode_path() {
	emulate -L zsh
	local LC_ALL=C input=$1 output='' character encoded
	local index
	for ((index = 1; index <= ${#input}; index += 1)); do
		character=${input[index]}
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

function __orchbee_preexec() {
	__ORCHBEE_COMMAND_ACTIVE=1
	printf '\033]133;C\007'
}

function __orchbee_precmd() {
	local exit_code=$?
	if (( __ORCHBEE_COMMAND_ACTIVE )); then
		printf '\033]133;D;%d\007' "$exit_code"
		__ORCHBEE_COMMAND_ACTIVE=0
	fi
	__orchbee_encode_path "$PWD"
	printf '\033]7;file://%s\007' "$REPLY"
	printf '\033]133;A\007'
}

add-zsh-hook preexec __orchbee_preexec
add-zsh-hook precmd __orchbee_precmd

function __orchbee_ssh_observation_eligible() {
	[[ -n ${ORCHBEE_AGENT_OBSERVATION_SOCKET-} && -S ${ORCHBEE_AGENT_OBSERVATION_SOCKET-} && -t 0 && -t 1 ]] || return 1
	local argument
	for argument in "$@"; do
		case $argument in
			-G|-O|-Q|-V|-W|-W*) return 1 ;;
		esac
		if [[ $argument =~ '^-[46AaCfGgKkMNnqsTtVvXxYy]+$' && $argument =~ '[NnfT]' ]]; then
			return 1
		fi
	done
}

function __orchbee_ssh_observation_nonce() {
	emulate -L zsh
	local nonce
	nonce=$(od -An -N16 -tx1 /dev/urandom 2>/dev/null | tr -d ' \n')
	if [[ ! $nonce == [a-f0-9]## ]]; then
		nonce="$$${RANDOM:-0}${RANDOM:-0}$(date +%s 2>/dev/null)"
		nonce=${nonce//[^A-Za-z0-9_-]/}
	fi
	REPLY=$nonce
}

if [[ -z ${aliases[ssh]-} && -z ${functions[ssh]-} && $(whence -w ssh 2>/dev/null) == 'ssh: command' ]]; then
	function ssh() {
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
