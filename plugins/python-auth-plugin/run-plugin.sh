#!/bin/bash

docker build -t mapi-python-rewrite-plugin .
docker run -it --rm --name mapi-plugin -p 9001:9001 mapi-python-rewrite-plugin
