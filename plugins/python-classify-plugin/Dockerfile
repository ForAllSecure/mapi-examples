FROM python:3.8
LABEL maintainer="ForAllSecure Team <mayhem4api@forallsecure.com>"
ARG port=50051
ENV MAPI_PLUGIN_PORT=$port

WORKDIR /workdir
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY *.proto .
COPY src src

# Generate source from .proto
RUN mkdir generated && python3 -m grpc_tools.protoc --python_out=generated --grpc_python_out=generated -I. response-classify-plugin.proto

EXPOSE $port

# command to run on container start
CMD [ "python3", "./src/plugin.py" ]
