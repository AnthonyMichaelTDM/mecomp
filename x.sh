#!/usr/bin/env bash

# Colors.
readonly WHITE="\033[1;97m" GREEN="\033[1;92m" RED="\033[1;91m" YELLOW="\033[1;93m" BLUE="\033[1;94m" OFF="\033[0m" TITLE="==============================================================>"

# Logging.
title() {
	printf "\n${BLUE}${TITLE}${WHITE} %s${OFF}\n" "$1"
}
fail() {
	printf "${RED}${TITLE}${WHITE} %s${OFF}\n" "$1"
	exit 1
}
ok() {
	printf "${GREEN}${TITLE}${WHITE} %s${OFF}\n" "$1"
}
finish() {
	printf "\n\n\n${GREEN}${TITLE}${WHITE} %s${OFF}\n" "MECOMP Build OK."
}

# Help message.
help() {
	echo "./x.sh [ARG]"
	echo ""
	echo "Lint/test/build all packages in the MECOMP repo."
	echo "Builds are done with --release mode."
	echo ""
	echo "Arguments:"
	echo "    c | clippy    lint all packages"
	echo "    t | test      test all packages"
	echo "    b | build     build all packages"
	echo "    a | all       do all the above"
	echo "    h | help      print help"
}

# Clippy.
clippy() {
	for i in {mecomp-storage,mecomp-core,mecomp-cli,mecomp-tui,mecomp-daemon,one-or-many,surrealqlx,surrealqlx-macros,surrealqlx-macros-impl}; do
		title "Clippy [${i}]"
		if cargo clippy -r -p ${i} --no-deps; then
			ok "Clippy [${i}] OK"
		else
			fail "Clippy [${i}] FAIL"
		fi
	done

}

# Test.
test() {
	for i in {mecomp-storage,mecomp-core,mecomp-cli,mecomp-tui,mecomp-daemon,one-or-many,surrealqlx-macros-impl}; do
		title "Test [${i}]"
		if cargo test -p ${i}; then
			ok "Test [${i}] OK"
		else
			fail "Test [${i}] FAIL"
		fi
	done

	# Special cases
	# ...
}

# Build.
build() {
	# Build the binaries.
	for i in {mecomp-cli,mecomp-tui,mecomp-daemon}; do
		title "Build [${i}]"
		if cargo build -r -p ${i}; then
			ok "Build [${i}] OK"
		else
			fail "Build [${i}] FAIL"
		fi
	done

	finish
	ls -al --color=always target/release/mecomp-daemon
	ls -al --color=always target/release/mecomp-cli
}

# Do everything.
all() {
	clippy
	test
	build
}

# Subcommands.
case $1 in
	'a'|'all') all;;
	'c'|'clippy') clippy;;
	't'|'test') test;;
	'b'|'build') build;;
	*) help;;
esac
