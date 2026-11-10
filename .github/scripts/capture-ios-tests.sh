#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--launch" ]]; then
    [[ $# -eq 4 ]] || { echo "Invalid launch arguments" >&2; exit 2; }
    simulator="$2"
    application="$3"
    capture="$4"

    launch_output="$(xcrun simctl launch \
        --stdout="$capture/stdout.log" --stderr="$capture/stderr.log" \
        "$simulator" "$application")"
    printf '%s\n' "$launch_output"
    prefix="$application: "
    [[ "$launch_output" == "$prefix"* ]] || { echo "Could not identify the launched app" >&2; exit 1; }
    pid="${launch_output#"$prefix"}"
    [[ "$pid" =~ ^[1-9][0-9]*$ ]] || { echo "Invalid simulator app PID" >&2; exit 1; }
    # File redirection makes simctl return before the app exits.
    exec gtail --pid="$pid" --follow=descriptor --sleep-interval=0.1 "$capture/stdout.log" >/dev/null
fi

[[ $# -eq 4 ]] || { echo "Usage: $0 SIMULATOR APPLICATION OUTPUT_PREFIX TIMEOUT" >&2; exit 2; }
simulator="$1"
application="$2"
output_prefix="$3"
duration="$4"

container="$(gtimeout 120s xcrun simctl get_app_container "$simulator" "$application" data)"
[[ -d "$container/Documents" ]] || { echo "Missing simulator app data directory" >&2; exit 1; }
capture="$(mktemp -d "$container/Documents/bn-ios-tests.XXXXXXXX")"

status=0
gtimeout "$duration" bash "$0" --launch "$simulator" "$application" "$capture" || status=$?
echo "Simulator launch and app wait status: $status"

if (( status != 0 )); then
    if ! gtimeout 120s xcrun simctl terminate "$simulator" "$application"; then
        echo "Could not terminate the simulator app after capture failed" >&2
    fi
fi

for stream in stdout stderr; do
    if [[ ! -f "$capture/$stream.log" ]] || ! cp "$capture/$stream.log" "$output_prefix.$stream.log"; then
        echo "Could not retrieve simulator $stream" >&2
        if (( status == 0 )); then
            status=1
        fi
    fi
done

if [[ -f "$output_prefix.stdout.log" ]]; then
    cat "$output_prefix.stdout.log"
fi
if [[ -f "$output_prefix.stderr.log" ]]; then
    cat "$output_prefix.stderr.log" >&2
fi

if (( status != 0 )); then
    exit "$status"
fi

if ! tail -n 1 "$output_prefix.stderr.log" | tr -d '\r ' | grep -x 0; then
    echo "Missing or nonzero simulator test exit marker" >&2
    exit 1
fi
