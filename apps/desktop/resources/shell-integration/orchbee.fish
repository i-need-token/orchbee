# Orchbee Fish integration. Source this file explicitly from a Fish startup file.

if set -q __orchbee_shell_integration_loaded
	return 0
end
set -g __orchbee_shell_integration_loaded 1
set -g __orchbee_command_active 0

function __orchbee_preexec --on-event fish_preexec
	set -g __orchbee_command_active 1
	printf '\e]133;C\a'
	__orchbee_agent_integration_launch "$argv[1]"
end

function __orchbee_agent_integration_launch
	set -l command_line "$argv[1]"
	set -l command_word (string replace -r '^[[:space:]]*([^[:space:];|&]+).*$' '$1' -- "$command_line")
	set -l executable (string replace -r '^.*/' '' -- "$command_word")
	set -l payload
	switch "$executable"
		case claude
			set payload eyJpbnRlZ3JhdGlvbklkIjoiY2xhdWRlLWNvZGUiLCJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6M30
		case codex
			set payload eyJpbnRlZ3JhdGlvbklkIjoiY29kZXgiLCJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6M30
		case grok
			set payload eyJpbnRlZ3JhdGlvbklkIjoiZ3JvayIsImtpbmQiOiJsYXVuY2giLCJ2ZXJzaW9uIjozfQ
		case opencode
			set payload eyJpbnRlZ3JhdGlvbklkIjoib3BlbmNvZGUiLCJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6M30
		case pi
			set payload eyJpbnRlZ3JhdGlvbklkIjoicGkiLCJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6M30
		case omp
			set payload eyJpbnRlZ3JhdGlvbklkIjoib21wIiwia2luZCI6ImxhdW5jaCIsInZlcnNpb24iOjN9
		case gemini
			set payload eyJpbnRlZ3JhdGlvbklkIjoiZ2VtaW5pIiwia2luZCI6ImxhdW5jaCIsInZlcnNpb24iOjN9
		case copilot
			set payload eyJpbnRlZ3JhdGlvbklkIjoiZ2l0aHViLWNvcGlsb3QiLCJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6M30
		case cline
			set payload eyJpbnRlZ3JhdGlvbklkIjoiY2xpbmUiLCJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6M30
		case qwen
			set payload eyJpbnRlZ3JhdGlvbklkIjoicXdlbi1jb2RlIiwia2luZCI6ImxhdW5jaCIsInZlcnNpb24iOjN9
		case '*'
			return 0
	end
	if test (type -t "$command_word" 2>/dev/null) != file
		return 0
	end
	printf '\e_orchbee.agent.v3;%s\e\\' "$payload"
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

function __orchbee_ssh_observation_eligible
	if not set -q ORCHBEE_AGENT_OBSERVATION_SOCKET_V3
		return 1
	end
	if not test -S "$ORCHBEE_AGENT_OBSERVATION_SOCKET_V3"; or not isatty stdin; or not isatty stdout
		return 1
	end
	for argument in $argv
		switch $argument
			case -G -O -Q -V -W '-W*'
				return 1
		end
		if string match -rq '^-[46AaCfGgKkMNnqsTtVvXxYy]+$' -- $argument; and string match -rq '[NnfT]' -- $argument
			return 1
		end
	end
	return 0
end

function __orchbee_ssh_observation_nonce
	set -l nonce (od -An -N16 -tx1 /dev/urandom 2>/dev/null | string replace -a ' ' '' | string collect)
	set nonce (string replace -a '\n' '' -- $nonce)
	if not string match -rq '^[a-f0-9]{32}$' -- $nonce
		set nonce "$fish_pid"(random)(random)(date +%s 2>/dev/null)
		set nonce (string replace -ra '[^A-Za-z0-9_-]' '' -- $nonce)
	end
	printf '%s' $nonce
end

if test (type -t ssh 2>/dev/null) = file
	function ssh
		if not __orchbee_ssh_observation_eligible $argv
			command ssh $argv
			return $status
		end
		set -l remote_socket "/tmp/orchbee-agent-v3-"(__orchbee_ssh_observation_nonce)".sock"
		command ssh \
			-o StreamLocalBindMask=0177 \
			-o StreamLocalBindUnlink=yes \
			-R "$remote_socket:$ORCHBEE_AGENT_OBSERVATION_SOCKET_V3" \
			$argv
	end
end
