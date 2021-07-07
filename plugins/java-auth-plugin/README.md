# Prerequisites

* Java 1.7 or higher
* `mapi` CLI 2.6.18 or higher [Installation](https://mayhem4api.forallsecure.com/docs/ch01-01-installation.html)
* Docker (Optional)


Select one of the development methods below.

# Developing

## Build the service

```shell
./gradlew installDist
```

## Running the plugin with `mapi`

Now we can run the plugin with the `mapi` CLI!

You can let `mapi` handle the lifecycle of starting and stopping your plugin
by passing the path to `./build/install/java-auth-plugin/bin/mapi-plugin-server`
(created by the `installDist` gradle task) to the `mapi run` command.


```shell
# Start a new fuzzing job named 'plugin-example' and run for 60 seconds
mapi run --url <API_URL> plugin-example \
  60 \
  <API_SPECIFICATION_PATH> \
  --experimental-plugin ./build/install/java-auth-plugin/bin/mapi-plugin-server
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
MAPI_PLUGIN_PORT=9001 ./build/install/java-auth-plugin/bin/mapi-plugin-server

9001
```
