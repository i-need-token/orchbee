# Orchbee's managed zsh startup. The Terminal Host points ZDOTDIR at this directory and keeps
# the person's own ZDOTDIR, if any, in ORCHBEE_ZSH_ZDOTDIR. This file puts it back, reads the
# person's own .zshenv where zsh would have, and loads Orchbee's prompt and command marks into an
# interactive shell. zsh then reads the person's .zprofile, .zshrc and .zlogin itself, as usual.
# No startup file of the person's is ever written.
builtin typeset -g __orchbee_integration=${ZDOTDIR}
if [[ -n ${ORCHBEE_ZSH_ZDOTDIR+set} ]]; then
	ZDOTDIR=${ORCHBEE_ZSH_ZDOTDIR}
	builtin unset ORCHBEE_ZSH_ZDOTDIR
else
	builtin unset ZDOTDIR
fi
# The start reaches only this shell: nothing a startup file starts inherits it.
builtin typeset -g __orchbee_start=${ORCHBEE_AGENT_START-}
builtin unset ORCHBEE_AGENT_START
{
	if [[ -r ${ZDOTDIR-$HOME}/.zshenv ]]; then
		builtin source -- "${ZDOTDIR-$HOME}/.zshenv"
	fi
} always {
	if [[ -o interactive && -r ${__orchbee_integration}/orchbee.zsh ]]; then
		builtin source -- "${__orchbee_integration}/orchbee.zsh"
	else
		builtin unset __orchbee_start
	fi
	builtin unset __orchbee_integration
}
