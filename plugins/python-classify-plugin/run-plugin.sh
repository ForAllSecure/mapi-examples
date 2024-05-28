#!/bin/bash

docker build -t mapi-python-classify-plugin .
docker run -it --rm --name classify-plugin -p 50051:50051 mapi-python-classify-plugin
