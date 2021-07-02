# Prerequisites

* Python 3.8 or higher
* `mapi` CLI 2.6.18 or higher [Installation](https://mayhem4api.forallsecure.com/docs/ch01-01-installation.html)
* Docker (Optional)


Select one of the development methods below. You must be

# Developing

## Create a local virtual environment:

```shell
python3 -m venv venv
source venv/bin/activate
python3 -m pip install -r requirements.txt
```

## Generate code for `.proto` file.

ℹ️ This will generate the source files from the `.proto` file

```shell
mkdir -p generated
python3 -m grpc_tools.protoc --python_out=generated --grpc_python_out=generated -I. request-rewrite-plugin.proto
```

The server code is located under [`src/plugin.py`](src/plugin.py)

## Running the plugin with `mapi`

Now we can run the plugin with the `mapi` CLI!


You can let `mapi` handle the lifecycle of starting and stopping your plugin
by passing the path to [`src/plugin.py`](src/plugin.py) to the `mapi run` command.


```shell
# Make sure the virtualenv is active
source venv/bin/activate

# Start a new fuzzing job named 'plugin-example' and run for 60 seconds
mapi run --url <API_URL> plugin-example \
  60 \
  <API_SPECIFICATION_PATH> \
  --experimental-plugin src/plugin.py
```

If you wish to debug the server while `mapi` is running in your IDE of favorite
debugging tool, you will need to start the server FIRST. Once the server is running,
you can pass the URL to `mapi run`:

```shell
# Set MAPI_PLUGIN_PORT=9001 to force the plugin to run on port 9001
# ... start your server ...

# Start a new fuzzing job named 'plugin-example' and run for 60 seconds
mapi run --url <API_URL> plugin-example \
  60 \
  <API_SPECIFICATION_PATH> \
  --experimental-plugin http://localhost:9001
```


## Other run options

ℹ️ A port may be manually specified with the `MAPI_PLUGIN_PORT` environment variable.

Run with a manually specified port (9001):

```shell
MAPI_PLUGIN_PORT=9001 python3 src/plugin.py

9001
```

Run with an automatically assigned port:

```shell
python3 src/plugin.py

59304
```

This will start the plugin in listening mode. The only output you should see is the port where
the server is listening.

## Running with Docker

A `Dockerfile` is included to demonstrate how to run the plugin in a Docker container. First
you must build the image:

```shell
docker build -t mapi-python-auth-plugin .
```

You can now run the server with the default port (`9001`).

```shell
docker run -it --rm --name mapi-plugin -p 9001:9001 mapi-python-auth-plugin

9001
```

With the docker container running, you can instruct `mapi` to use it for request
rewrites by passing the url to the `mapi run` command:

```shell
docker run -it --rm -d --name mapi-plugin -p 9001:9001 mapi-python-auth-plugin

# Start a new fuzzing job named 'plugin-example' and run for 60 seconds
mapi run --url <API_URL> plugin-example \
  60 \
  <API_SPECIFICATION_PATH> \
  --experimental-plugin http://localhost:9001
```
