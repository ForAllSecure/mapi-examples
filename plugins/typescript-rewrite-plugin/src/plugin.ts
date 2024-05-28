import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import { sendUnaryData, ServerUnaryCall, UntypedServiceImplementation } from '@grpc/grpc-js';

// Load your protobuf
const PROTO_PATH = './request-rewrite-plugin.proto';
const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
    keepCase: true,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true
});
const protoDescriptor = grpc.loadPackageDefinition(packageDefinition) as unknown as { mapi: { rewrite: { RewritePlugin: { service: grpc.ServiceDefinition<grpc.UntypedServiceImplementation> } } } };
const requestRewritePlugin = protoDescriptor.mapi.rewrite.RewritePlugin;

console.log(JSON.stringify(protoDescriptor, null, 2));


// Implement the RewritePlugin service
class RewritePluginServicer implements UntypedServiceImplementation {
    [name: string]: grpc.UntypedHandleCall;
    public rewrite(call: ServerUnaryCall<any, any>, callback: sendUnaryData<any>): void {
        callback(null, call.request); // Simply returning the request as response for now; try something interesting!
    }
}

function main() {
    const server = new grpc.Server();
    server.addService(requestRewritePlugin.service, new RewritePluginServicer());

    const plugin_port = process.env.MAPI_PLUGIN_PORT || '50051';

    server.bindAsync(`127.0.0.1:${plugin_port}`, grpc.ServerCredentials.createInsecure(), (err, port) => {
        if (err) {
            console.error(err);
            return;
        }
        console.log(`Server running on port ${port}`);
    });
}

// Start the server
main();

