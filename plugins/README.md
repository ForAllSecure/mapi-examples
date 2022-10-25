# Request Rewrite Plugin Examples

[Request Rewrite plugins](https://mayhem4api.forallsecure.com/docs/rewrite.html)
allow you to intercept and modify every request sent to your API from the Mayhem for API
CLI. These plugins allow you to apply custom authentication, request signing
and other modifications to every request that are not available out of the box.

### [python-auth-plugin](python-auth-plugin)

This example implements a service that adds an `Authorization` header to every request.

### [java-auth-plugin](java-auth-plugin)

This example implements the same service as `python-auth-plugin` but written in Java.
