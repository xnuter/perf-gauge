#!/bin/sh

# e.g. if the session runs with a token, and we'd like to refresh the token once it expired:
while true; do
  TOKEN=$(refresh_token)

  # once perf-gauge prints "403 Unauthorized" SIGPIPE will be sent to stop execution
  perf-gauge ... -H "Authorization: ${TOKEN}" | seq /Unauthorized/q
done