#!/bin/sh

tmux new-session -d -s "java1" ./perf-gauge/start-java1.sh

tmux new-session -d -s "java2" ./perf-gauge/start-java2.sh
