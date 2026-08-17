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
