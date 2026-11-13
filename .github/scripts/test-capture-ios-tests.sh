#!/usr/bin/env bash
set -euo pipefail

root="$PWD"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAL_TIMEOUT="$(command -v gtimeout || command -v timeout)"
REAL_TAIL="$(command -v gtail || command -v tail)"
REAL_CP="$(command -v cp)"
REAL_CAT="$(command -v cat)"
export REAL_TIMEOUT REAL_TAIL REAL_CP REAL_CAT
fixture="$(mktemp -d "$root/.ios capture test.XXXXXXXX")"
cleanup() {
    if [[ "${MOCK_SCENARIO:-}" == "live-marker" && -f "$fixture/pid" ]]; then
        pid="$(<"$fixture/pid")"
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid"
        fi
    fi
    rm -f "$fixture/bin/xcrun" "$fixture/bin/gtimeout" "$fixture/bin/gtail" "$fixture/bin/cp" "$fixture/bin/cat" \
        "$fixture/pid" "$fixture/terminations" "$fixture/live-wait-status"
    rm -f "$fixture"/results/*
    for capture in "$fixture"/container/Documents/bn-ios-tests.*; do
        if [[ -d "$capture" ]]; then
            rm -f "$capture/stdout.log" "$capture/stderr.log" "$capture/launcher.log"
            rmdir "$capture"
        fi
    done
    rmdir "$fixture/bin" "$fixture/results" "$fixture/container/Documents" "$fixture/container" "$fixture"
}
trap cleanup EXIT
trap 'echo "Control failed in ${MOCK_SCENARIO:-setup} at line $LINENO" >&2' ERR
mkdir -p "$fixture/bin" "$fixture/results" "$fixture/container/Documents"
export MOCK_ROOT="$fixture"
export PATH="$fixture/bin:$PATH"

cat >"$fixture/bin/gtimeout" <<'EOF'
#!/usr/bin/env bash
exec "$REAL_TIMEOUT" "$@"
EOF
cat >"$fixture/bin/gtail" <<'EOF'
#!/usr/bin/env bash
if [[ "$MOCK_SCENARIO" == "wait-error" ]]; then
    echo "process wait failed" >&2
    exit 42
fi
if [[ "$MOCK_SCENARIO" == "live-marker" ]]; then
    stdout="${!#}"
    stderr="${stdout%stdout.log}stderr.log"
    until [[ -s "$stderr" ]]; do
        sleep 0.1
    done
    pid="$(<"$MOCK_ROOT/pid")"
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "simulated app exited before the wait" >&2
        exit 1
    fi
    # Start the short timeout only after the marker is present and the app is alive.
    status=0
    "$REAL_TIMEOUT" 1s "$REAL_TAIL" "$@" || status=$?
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "simulated app exited during the wait" >&2
        exit 1
    fi
    printf '%s\n' "$status" >"$MOCK_ROOT/live-wait-status"
    exit "$status"
fi
exec "$REAL_TAIL" "$@"
EOF
cat >"$fixture/bin/cp" <<'EOF'
#!/usr/bin/env bash
if [[ "$MOCK_SCENARIO" == "copy-error" && "$1" == */stderr.log ]]; then
    echo "simulated copy failure" >&2
    exit 1
fi
exec "$REAL_CP" "$@"
EOF
cat >"$fixture/bin/cat" <<'EOF'
#!/usr/bin/env bash
if [[ "$MOCK_SCENARIO" == "launcher-copy-error" && "${1:-}" == */launcher.log ]]; then
    echo "simulated launcher output failure" >&2
    exit 1
fi
exec "$REAL_CAT" "$@"
EOF
cat >"$fixture/bin/xcrun" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == "simctl" ]]
case "$2" in
    get_app_container)
        printf '%s\n' "$MOCK_ROOT/container"
        ;;
    launch)
        stdout="${3#--stdout=}"
        stderr="${4#--stderr=}"
        app="$6"
        case "$MOCK_SCENARIO" in
            launch-error)
                echo "simulator launch failed" >&2
                exit 45
                ;;
            malformed)
                echo "unexpected launch output"
                exit 0
                ;;
            invalid-pid)
                printf '%s: not-a-pid\n' "$app"
                exit 0
                ;;
            extra-output)
                printf '%s: 123\nextra output\n' "$app"
                exit 0
                ;;
        esac
        if [[ "$MOCK_SCENARIO" != "missing-stdout" ]]; then
            : >"$stdout"
        fi
        if [[ "$MOCK_SCENARIO" != "missing-stderr" ]]; then
            : >"$stderr"
        fi
        bash -c '
            scenario="$1"
            stdout="$2"
            stderr="$3"
            if [[ "$scenario" != "missing-stdout" ]]; then
                printf "Running 30 tests\n" >>"$stdout"
            fi
            case "$scenario" in
                live-marker)
                    printf "0\n" >>"$stderr"
                    exec sleep 120
                    ;;
                wait-error)
                    printf "0\n" >>"$stderr"
                    exec sleep 2
                    ;;
                missing-stdout)
                    exec sleep 2
                    ;;
                test-failure)
                    printf "[ FAILED ]\n" >>"$stdout"
                    printf "1\n" >>"$stderr"
                    ;;
                missing-marker)
                    printf "[ PASSED ] 30 tests\n" >>"$stdout"
                    ;;
                missing-stderr)
                    printf "[ PASSED ] 30 tests\n" >>"$stdout"
                    ;;
                marker-whitespace)
                    printf "[ PASSED ] 30 tests\n" >>"$stdout"
                    printf " 0 \r\n" >>"$stderr"
                    ;;
                after-marker)
                    printf "[ PASSED ] 30 tests\n" >>"$stdout"
                    printf "0\nunexpected output\n" >>"$stderr"
                    ;;
                *)
                    if [[ "$scenario" != "fast" ]]; then
                        sleep 0.3
                    fi
                    printf "[ PASSED ] 30 tests\n" >>"$stdout"
                    printf "0\n" >>"$stderr"
                    ;;
            esac
        ' _ "$MOCK_SCENARIO" "$stdout" "$stderr" </dev/null >/dev/null 2>&1 &
        pid=$!
        printf '%s\n' "$pid" >"$MOCK_ROOT/pid"
        if [[ "$MOCK_SCENARIO" == "fast" ]]; then
            wait "$pid"
        fi
        printf '%s: %s\n' "$app" "$pid"
        ;;
    terminate)
        if [[ -f "$MOCK_ROOT/pid" ]]; then
            [[ "$3" == "SIMULATOR" && "$4" == "example.test" ]]
            echo "$MOCK_SCENARIO" >>"$MOCK_ROOT/terminations"
            if [[ "$MOCK_SCENARIO" == "live-marker" ]]; then
                kill "$(<"$MOCK_ROOT/pid")"
            fi
        else
            echo "no app to terminate" >&2
            exit 1
        fi
        ;;
    *)
        echo "unexpected simctl command" >&2
        exit 2
        ;;
esac
EOF
chmod +x "$fixture/bin/xcrun" "$fixture/bin/gtimeout" "$fixture/bin/gtail" "$fixture/bin/cp" "$fixture/bin/cat"

run_case() {
    local scenario="$1" expected="$2" duration="${3:-60s}"
    export MOCK_SCENARIO="$scenario"
    rm -f "$fixture/pid"
    local status=0
    bash "$script_dir/capture-ios-tests.sh" SIMULATOR example.test "$fixture/results/$scenario" "$duration" \
        >"$fixture/results/$scenario.output" 2>&1 || status=$?
    if [[ "$status" != "$expected" ]]; then
        cat "$fixture/results/$scenario.output" >&2
        echo "$scenario: expected status $expected, got $status" >&2
        exit 1
    fi
    if [[ -f "$fixture/pid" ]]; then
        local pid
        pid="$(cat "$fixture/pid")"
        for ((attempt = 0; attempt < 50; attempt++)); do
            if ! kill -0 "$pid" 2>/dev/null; then
                break
            fi
            sleep 0.1
        done
        if kill -0 "$pid" 2>/dev/null; then
            echo "$scenario: simulated app did not exit" >&2
            exit 1
        fi
    fi
}

marker_is_zero() {
    tail -n 1 "$1" | tr -d ' \r' | grep -q '^0$'
}

for scenario in normal fast marker-whitespace; do
    run_case "$scenario" 0
    grep -q 'Running 30 tests' "$fixture/results/$scenario.stdout.log"
    grep -q '\[ PASSED \] 30 tests' "$fixture/results/$scenario.stdout.log"
    marker_is_zero "$fixture/results/$scenario.stderr.log"
    pid="$(cat "$fixture/pid")"
    if [[ "$(tail -n 1 "$fixture/results/$scenario.stdout.log")" != "example.test: $pid" ]]; then
        echo "$scenario: missing launcher PID in stdout artifact" >&2
        exit 1
    fi
done

for scenario in test-failure missing-marker after-marker; do
    run_case "$scenario" 1
    if marker_is_zero "$fixture/results/$scenario.stderr.log"; then
        echo "$scenario: invalid marker was accepted" >&2
        exit 1
    fi
done

run_case missing-stderr 1
run_case missing-stdout 1
run_case copy-error 1
run_case launcher-copy-error 1
grep -q 'Could not retrieve simulator launcher output' "$fixture/results/launcher-copy-error.output"
run_case malformed 1
run_case invalid-pid 1
run_case extra-output 1
run_case launch-error 45
run_case wait-error 42
run_case live-marker 124
grep -qx '124' "$fixture/live-wait-status"
marker_is_zero "$fixture/results/live-marker.stderr.log"
grep -qx 'live-marker' "$fixture/terminations"
echo "All simulator capture controls passed"
