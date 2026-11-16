# Orchbee's managed bash startup. The Terminal Host starts an interactive bash with --rcfile naming
# this file instead of a login shell's -l, and sets ORCHBEE_BASH_LOGIN when the launch asked for a
# login shell. This file reads the person's startup files in the order bash itself would have, then
# adds Orchbee's prompt and command marks. It runs in bash 3.2, as macOS ships it. No startup file
# of the person's is ever written. The marks use the managed PowerShell's private grammar:
#   before each prompt: APC ready, then the prompt ends with OSC 133 P and OSC 133 B;
#   when a line runs: OSC 133 C, then APC start;<token> once ORCHBEE_RUN_TOKEN holds it;
#   at the next prompt: OSC 133 D;<status>, APC finish;<token>;<status>, and the token unset.
# A line Orchbee submits starts with ": orchbee-run:<32 hex digits>; ", a no-op that names its
# token; that line is removed from the history. The Agent start the launch carried runs once,
# before the first prompt, after every startup file and every other prompt command.
# Emulating a login shell keeps everything a login shell reads, but bash itself is not one:
# `shopt login_shell` is off, `logout` refuses, and ~/.bash_logout does not run.

__orchbee_start=${ORCHBEE_AGENT_START-}
builtin unset ORCHBEE_AGENT_START
if [ -n "${ORCHBEE_BASH_LOGIN+set}" ]; then
	builtin unset ORCHBEE_BASH_LOGIN
	if [ -r /etc/profile ]; then
		builtin . /etc/profile
	fi
	if [ -r "$HOME/.bash_profile" ]; then
		builtin . "$HOME/.bash_profile"
	elif [ -r "$HOME/.bash_login" ]; then
		builtin . "$HOME/.bash_login"
	elif [ -r "$HOME/.profile" ]; then
		builtin . "$HOME/.profile"
	fi
elif [ -r "$HOME/.bashrc" ]; then
	builtin . "$HOME/.bashrc"
fi

__orchbee_mint() {
	__orchbee_count=$((__orchbee_count + 1))
	builtin printf -v __orchbee_id '%s%08x' "$__orchbee_seed" "$__orchbee_count"
}

__orchbee_precmd() {
	local code=$?
	if [ -n "$__orchbee_run" ]; then
		builtin printf '\033]133;D;%s\007\033_orchbee.command.v1;finish;%s;%s\033\\' "$code" "$__orchbee_run" "$code"
		__orchbee_run=
		builtin unset ORCHBEE_RUN_TOKEN
	fi
	return $code
}

# The last prompt command: it marks the prompt the person's prompt commands left, runs the Agent
# start once, and arms the command mark for the next line.
__orchbee_prompt() {
	local code=$?
	case $PS1 in
	*"$__orchbee_ps1") ;;
	*) PS1=$PS1$__orchbee_ps1 ;;
	esac
	if [ -n "$__orchbee_start" ]; then
		local line=$__orchbee_start
		__orchbee_start=
		__orchbee_mint
		builtin export ORCHBEE_RUN_TOKEN=$__orchbee_id
		builtin printf '\033_orchbee.command.v1;start;%s\033\\' "$__orchbee_id"
		builtin eval "$line"
		builtin printf '\033_orchbee.command.v1;finish;%s;%s\033\\' "$__orchbee_id" "$?"
		builtin unset ORCHBEE_RUN_TOKEN
	fi
	builtin printf '\033_orchbee.command.v1;ready\033\\'
	__orchbee_armed=1
	return $code
}

# The DEBUG trap: the first command of a line that runs marks it. A blank line runs only prompt
# commands, which never count.
__orchbee_preexec() {
	if [ -z "$__orchbee_armed" ] || [ -n "${COMP_LINE-}" ]; then
		return 0
	fi
	case $BASH_COMMAND in
	__orchbee_* | __bp_*)
		__orchbee_armed=
		return 0
		;;
	esac
	case "
${PROMPT_COMMAND//;/
}
" in
	*"
$BASH_COMMAND
"*)
		__orchbee_armed=
		return 0
		;;
	esac
	__orchbee_armed=
	local id=
	case $BASH_COMMAND in
	": orchbee-run:"*)
		id=${BASH_COMMAND#": orchbee-run:"}
		case $id in
		*[!0-9a-f]*) id= ;;
		esac
		if [ ${#id} -ne 32 ]; then
			id=
		elif [[ -o history ]]; then
			local entry number
			entry=$(HISTTIMEFORMAT= builtin history 1)
			entry=${entry#"${entry%%[! ]*}"}
			number=${entry%% *}
			entry=${entry#"$number"}
			entry=${entry#"${entry%%[! ]*}"}
			case $entry in
			": orchbee-run:$id; "*) builtin history -d "$number" ;;
			esac
		fi
		;;
	esac
	if [ -z "$id" ]; then
		__orchbee_mint
		id=$__orchbee_id
	fi
	__orchbee_run=$id
	builtin export ORCHBEE_RUN_TOKEN=$id
	builtin printf '\033]133;C\007\033_orchbee.command.v1;start;%s\033\\' "$id"
}

if [[ $- == *i* ]] && [ -z "${__orchbee_seed-}" ]; then
	__orchbee_run=
	__orchbee_armed=
	__orchbee_count=0
	__orchbee_ps1='\[\e]133;P\a\e]133;B\a\]'
	# Run tokens are this seed, read once, and a counter, so every token is unique and nonzero.
	__orchbee_seed=$(command od -An -N12 -tx1 /dev/urandom 2>/dev/null)
	__orchbee_seed=${__orchbee_seed//[!0-9a-f]/}
	if [ ${#__orchbee_seed} -ne 24 ]; then
		builtin printf -v __orchbee_seed '%04x%04x%04x%04x%04x%04x' $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM
	fi
	if [[ $(builtin declare -p PROMPT_COMMAND 2>/dev/null) == "declare -a"* ]]; then
		PROMPT_COMMAND=(__orchbee_precmd "${PROMPT_COMMAND[@]}" __orchbee_prompt)
	else
		PROMPT_COMMAND="__orchbee_precmd${PROMPT_COMMAND:+
$PROMPT_COMMAND}
__orchbee_prompt"
	fi
	# Another DEBUG trap keeps running, after this one.
	__orchbee_trap=$(builtin trap -p DEBUG)
	__orchbee_trap=${__orchbee_trap#"trap -- '"}
	__orchbee_trap=${__orchbee_trap%"' DEBUG"}
	__orchbee_trap=${__orchbee_trap//"'\\''"/"'"}
	builtin trap "__orchbee_preexec \"\$_\"${__orchbee_trap:+; $__orchbee_trap}" DEBUG
	builtin unset __orchbee_trap
else
	builtin unset __orchbee_start
fi
