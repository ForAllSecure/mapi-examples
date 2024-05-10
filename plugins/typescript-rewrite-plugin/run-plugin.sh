#!/bin/bash

npm install
npx tsc
nohup node dist/plugin.js &

echo "Server is now running"
