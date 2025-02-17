#!/usr/bin/env bash

# Finds the longest PATH and file with
# the most amount of lines in this repo.
# This is used for left-padding the filename
# in the `core/src/logger.rs` file.

# Exit on failure.
set -e

# `cd` to root.
[[ $PWD == */mecomp/scripts ]] && cd ..
[[ $PWD == */mecomp ]]

# Use `fd` if found.
if [[ -f /usr/bin/fd ]]; then
	FIND=$(fd .*.rs "daemon" "core" "storage" "analysis" "one-or-many" "mpris")
else
	FIND=$(find "daemon" "core" "storage" "analysis" "one-or-many" "mpris" -type f -iname *.rs)
fi

# PATH.
echo "Longest PATH"
echo "$FIND" | awk '{ print length(), $0 | "sort -n" }' | tail -n 1

# Lines.
echo
echo "Most lines"
wc -l $FIND | sort -h | tail -n 2 | head -n 1

# echo "Shortest PATH"
# echo "$FIND" | awk '{ print length(), $0 | "sort -n" }' | head -n 1
# echo "Least lines"
# wc -l $FIND | sort -h | head -n 2 | tail -n 1
