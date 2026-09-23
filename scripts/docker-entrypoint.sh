#!/bin/sh
set -eu

role="${1:-api}"
if [ "$#" -gt 0 ]; then shift; fi
case "$role" in
  api) exec attricat-api "$@" ;;
  file-worker) exec attricat-file-worker "$@" ;;
  migrate) exec attricat-migrate "$@" ;;
  acli) exec acli "$@" ;;
  *) exec "$role" "$@" ;;
esac
