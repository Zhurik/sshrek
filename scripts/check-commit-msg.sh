#!/usr/bin/env bash
# Conventional Commits, the subset this repository already uses:
#
#   feat: add a thing          fix(picker): stop hanging without a tty
#   chore: release v0.2.1      refactor!: drop the old state format
#
# Merge, revert and the commits `git commit --fixup`/`--squash` write are left
# alone: their first line is not ours to choose.
set -euo pipefail

msg_file="$1"
subject="$(sed -n '1p' "$msg_file")"

case "$subject" in
	"Merge "* | "Revert "* | "fixup!"* | "squash!"* | "amend!"*) exit 0 ;;
esac

types='build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test'
if printf '%s\n' "$subject" | grep -Eq "^($types)(\([a-z0-9._/-]+\))?!?: .+"; then
	exit 0
fi

cat >&2 <<MSG
Not a conventional commit subject:

    $subject

Expected <type>[(scope)][!]: <summary>, where type is one of:
    ${types//|/, }

For example:
    feat(picker): remember the last query
    fix: write the end event when ssh is killed
MSG
exit 1
