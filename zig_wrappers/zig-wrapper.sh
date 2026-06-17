#!/bin/bash
CMD="$1"
shift

args=()
skip=0
for ((i=1; i<=$#; i++)); do
    if [ $skip -gt 0 ]; then
        skip=$((skip - 1))
        continue
    fi
    arg="${!i}"
    if [[ "$arg" == --target=* ]]; then
        continue
    elif [[ "$arg" == "--target" ]] || [[ "$arg" == "-target" ]]; then
        skip=1
        continue
    else
        args+=("$arg")
    fi
done

ZIG_TARGET="${ZIG_TARGET:-x86_64-linux-gnu.2.17}"

if [ "$CMD" = "ar" ]; then
    exec zig ar "${args[@]}"
elif [ "$CMD" = "c++" ]; then
    exec zig c++ -target "$ZIG_TARGET" "${args[@]}"
else
    exec zig cc -target "$ZIG_TARGET" "${args[@]}"
fi
