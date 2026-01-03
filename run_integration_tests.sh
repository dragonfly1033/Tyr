#!/bin/bash

wd=$(pwd)
compile=$wd/target/debug/tyrc
test_dir=$wd/integration/tests
out_dir=$wd/integration/out

RED=$(tput setaf 1)
GREEN=$(tput setaf 2)
NORMAL=$(tput sgr0)

passed=0
failed=0

for t in $(find "$test_dir" -maxdepth 1 -mindepth 1 -type d -exec basename {} \;); do
    tmp=$(mktemp --suffix=""_tyr)

    printf "%s .... " "$t"
    "$compile" --file "$test_dir/$t/policies.tyr" --parent "$out_dir" --name policies
    cp "$test_dir/$t/main.rs" "$out_dir/src"
    cd "$out_dir" || exit 1
    if cargo test --color=always -- --color=always &> "$tmp"; then
        printf "%s\n" "$GREEN pass$NORMAL"
        passed=$((passed + 1))
        rm "$tmp"
    else
        printf "%s\n" "$RED FAILED $NORMAL"
        failed=$((failed + 1))
        cat "$tmp"
        rm "$tmp"
    fi
    cd "$wd" || exit 1
done

total=$((passed + failed))

printf "\n%s\n" "Ran $total tests. Passed: $GREEN $passed $NORMAL, Failed: $RED $failed $NORMAL"
