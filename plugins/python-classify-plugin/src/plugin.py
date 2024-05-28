#!/usr/bin/env python

import sys
import os
from concurrent import futures
import grpc
import numpy as np

sys.path.append("generated")
from response_classify_plugin_pb2 import Response, Issues
from response_classify_plugin_pb2_grpc import ClassifyPluginServicer, add_ClassifyPluginServicer_to_server


# Based on this info: https://github.com/googleapis/googleapis/blob/master/google/rpc/code.proto

GrpcToHTTPDict = {
    '0':'200',
    '1':'499',
    '2':'500',
    '3':'400',
    '4':'504',
    '5':'404',
    '6':'409',
    '7':'403',
    '8':'429',
    '9':'400',
    '10':'409',
    '11':'400',
    '12':'501',
    '13':'500',
    '14':'503',
    '15':'500',
    '16':'401'
}

HTTPToProblemDict = {
    '200':'OK',
    '499':'Canceled',
    '500':'UnknownError',
    '400':'InvalidArgument',
    '504':'DeadlineExceeded',
    '404':'NotFound',
    '409':'AlreadyExists',
    '403':'PermissionDenied',
    '429':'ResourceExhausted',
    '400':'FailedPrecondition',
    '409':'Aborted',
    '400':'OutOfRange',
    '501':'Unimplemented',
    '500':'InternalError',
    '503':'Unavailable',
    '500':'DataLoss',
    '401':'Unauthenticated'
}

## implement the ClassifyPlugin interface
class ClassifyPluginServicer(ClassifyPluginServicer):
    def Classify(self, response, context):
        body = str(response.body)
        grpcStatusStart = body.find('grpc-status:') + len('grpc-status:')
        grpcStatusEnd = body.find('\\', grpcStatusStart)
        grpcStatus = body[grpcStatusStart:grpcStatusEnd]
        HTTPResponse = GrpcToHTTPDict[grpcStatus]
        issues = Issues()
        if HTTPResponse[:1] == '5':
            issues.issues.append(Issues.Issue(summary=str(HTTPResponse + ': ' + HTTPToProblemDict[HTTPResponse])))
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
