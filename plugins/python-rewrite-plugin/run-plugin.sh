#!/bin/bash

docker build -t mapi-python-rewrite-plugin .
docker run -d -it --rm --name rewrite-plugin -p 9001:9001 mapi-python-rewrite-plugin 
