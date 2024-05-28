#!/bin/bash

npm install
npx tsc
node dist/plugin.js &

echo "Server is now running"
