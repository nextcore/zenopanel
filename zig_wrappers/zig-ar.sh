#!/bin/bash
exec $(dirname "$0")/zig-wrapper.sh ar "$@"
