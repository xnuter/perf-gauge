#!/bin/sh

tmux new-session -d -s "py1" ./perf-gauge/start-py1.sh

tmux new-session -d -s "py2" ./perf-gauge/start-py2.sh
