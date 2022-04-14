#!/usr/bin/env python

import sys
import os
from concurrent import futures
import grpc

sys.path.append("generated")
from response_classify_plugin_pb2 import Response, Issues
from response_classify_plugin_pb2_grpc import ClassifyPluginServicer, add_ClassifyPluginServicer_to_server

## implement the ClassifyPlugin interface
class ClassifyPluginServicer(ClassifyPluginServicer):
    def Classify(self, response, context):
        print(response.body)
        print(response.status)
        issues = Issues()
        return issues

def get_port():
    """
    The server can run on a manually specified port, using the `MAPI_PLUGIN_PORT` environment
    variable. If the variable is not set, then the server will start on a port randomly assigned
    by the OS.
    """
    return os.getenv('MAPI_PLUGIN_PORT') or 0

if __name__ == '__main__':
    ## boot up the gRPC server
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    add_ClassifyPluginServicer_to_server(ClassifyPluginServicer(), server)
    port = server.add_insecure_port(f'0.0.0.0:{get_port()}')
    server.start()

    ## inform mapi of the port we're listening on
    print(port)
    sys.stdout.flush()

    server.wait_for_termination()
