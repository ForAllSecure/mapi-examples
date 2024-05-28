#!/bin/bash

PID=$(pgrep -f 'node dist/plugin.js')

if [ -z "$PID" ]; then
    echo "Server not running."
else
    kill $PID
    rm -rf dist
    echo "Server with PID $PID has been stopped."
fi