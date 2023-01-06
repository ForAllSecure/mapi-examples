FROM python:3.11
LABEL maintainer="ForAllSecure Team <mayhem4api@forallsecure.com>"
ARG port=9001
ENV MAPI_PLUGIN_PORT=$port

WORKDIR /workdir
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY *.proto .
COPY src src

# Generate source from .proto
RUN mkdir generated && python3 -m grpc_tools.protoc --python_out=generated --grpc_python_out=generated -I. request-rewrite-plugin.proto

EXPOSE $port

# command to run on container start
CMD [ "python3", "./src/plugin.py" ]
