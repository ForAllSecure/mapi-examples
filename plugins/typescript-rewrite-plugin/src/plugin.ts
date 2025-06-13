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
        // Here we will demonstrate overwriting simple auth header
        const requestMetadata = call.metadata;
        // const auth = requestMetadata.get('Authorization');
        // if (auth && auth.length > 0) {
        // Don't need to get/check auth, just overwrite it or set a new one if it doesn't exist
        requestMetadata.set('Authorization', 'Bearer new_token_value');
        // }
        
        // Send the metadata
        call.sendMetadata(requestMetadata);

        // Return the modified request as the response
        callback(null, call.request);
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

