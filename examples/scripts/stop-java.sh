#!/bin/sh

tmux kill-session -t java1
tmux kill-session -t java2

ps -ef | grep java | grep -v grep | awk {'print $2'} | xargs kill -9

