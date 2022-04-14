#!/usr/bin/env python

import sys
import os
import itertools
from concurrent import futures

from base64 import b64encode
from random import choice
import grpc


sys.path.append("generated")
from request_rewrite_plugin_pb2 import Request
from request_rewrite_plugin_pb2_grpc import RewritePluginServicer, add_RewritePluginServicer_to_server

# content_types = [b'application/grpc-web', b'application/grpc-web+proto', b'application/grpc-web-text', b'application/grpc-web-text+proto']

class CustomRewritePluginServicer(RewritePluginServicer):
    def __init__(self):
        """
        When mapi is run with concurrency > 1, the server will be called concurrently as well.
        Using itertools.count() allows us to track requests count atomically.
        """
        self.request_count = itertools.count()

    def Rewrite(self, request, context):
        iter = 0
        for h in request.headers:
            if h.name == b'content-type':
                #request.headers[iter].value = b'application/grpc-web'
                request.headers[iter].value = b'application/grpc-web-text'
                #request.headers[iter].value = choice(content_types)
            iter += 1
        newBody = b64encode(request.body)
        request.body = newBody
        #print(request)
        return request



def get_port():
    """
    The server can run on a manually specified port, using the `MAPI_PLUGIN_PORT` environment
    variable. If the variable is not set, then the server will start on a port randomly assigned
    by the OS.
    """
    return os.getenv('MAPI_PLUGIN_PORT') or 0


if __name__ == '__main__':
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    add_RewritePluginServicer_to_server(CustomRewritePluginServicer(), server)

    # Bind to a port. Use the returned value to account for a randomly assigned
    # port.
    port = server.add_insecure_port(f'0.0.0.0:{get_port()}')

    # Start the server
    server.start()

    # inform mapi of the port we're listening on
    print(port)
    sys.stdout.flush()

    server.wait_for_termination()
