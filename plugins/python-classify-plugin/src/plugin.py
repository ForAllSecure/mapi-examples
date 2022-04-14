#!/usr/bin/env python

import sys
from concurrent import futures
import grpc

sys.path.append("generated")
import response_classify_plugin_pb2
import response_classify_plugin_pb2_grpc

## implement the ClassifyPlugin interface
class ClassifyPluginServicer(response_classify_plugin_pb2_grpc.ClassifyPluginServicer):
    def Classify(self, response, context):
        print(response.body)
        print(response.status)
        issues = response_classify_plugin_pb2.Issues()
        return issues

if __name__ == '__main__':
    ## boot up the gRPC server
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    response_classify_plugin_pb2_grpc.add_ClassifyPluginServicer_to_server(ClassifyPluginServicer(), server)
    server.add_insecure_port('127.0.0.1:50051')
    server.start()

    ## inform mapi of the port we're listening on
    print("50051")
    sys.stdout.flush()

    server.wait_for_termination()
