# Orchbee Fish integration. Source this file explicitly from a Fish startup file.

if set -q __orchbee_shell_integration_loaded
	return 0
end
set -g __orchbee_shell_integration_loaded 1
set -g __orchbee_command_active 0

function __orchbee_preexec --on-event fish_preexec
	set -g __orchbee_command_active 1
	printf '\e]133;C\a'
end

function __orchbee_postexec --on-event fish_postexec
	set -l exit_code $status
	if test $__orchbee_command_active -eq 1
		printf '\e]133;D;%d\a' $exit_code
		set -g __orchbee_command_active 0
	end
end

function __orchbee_prompt --on-event fish_prompt
	set -l encoded_path (string escape --style=url -- $PWD)
	set encoded_path (string replace -a '%2F' '/' -- $encoded_path)
	printf '\e]7;file://%s\a' $encoded_path
	printf '\e]133;A\a'
end
