# Prerequisites

* Node.js
* `mapi` CLI [Installation](https://mayhem4api.forallsecure.com/docs/ch01-01-installation.html)

Select one of the development methods below.

# Developing

## Install NPM dependencies.

```shell
npm install
```

## Transpile the TypeScript code into Javascript

Transpile configuration is kept under the `tsconfig.json` file.

```shell
npx tsc
```

The server code is located under [`src/plugin.ts`](src/plugin.ts)

## Running the plugin with `mapi`

Now we can run the plugin with the `mapi` CLI!

For Typescript/Javascript, the server needs to be running first. You can do this with `node`.
Once the server is running, you can pass the URL to `mapi run`:

```shell
# Start the server

node dist/plugin.js &

# Start a new fuzzing job named 'plugin-example' and run for 60 seconds
mapi run --url <API_URL> plugin-example \
  60 \
  <API_SPECIFICATION_PATH> \
  --rewrite-plugin http://localhost:50051
```

## Putting it all together

There are two convenience scripts in this folder that automate starting and stopping the server.

```shell
# Start the server

./run-plugin.sh

# Start your mapi job
mapi run --url <API_URL> plugin-example \
  60 \
  <API_SPECIFICATION_PATH> \
  --rewrite-plugin http://localhost:50051

# Stop the server
./stop-plugin.sh
```

## Other run options

ℹ️ A port may be manually specified with the `MAPI_PLUGIN_PORT` environment variable.

Run with a manually specified port (9001):

```shell
MAPI_PLUGIN_PORT=9001 node dist/plugin.js &

Server running on port 9001
```