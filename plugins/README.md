# Request Rewrite Plugin Examples

[Request Rewrite plugins](https://app.mayhem.security/docs/api-testing/guides/rewrite-plugin/)
allow you to intercept and modify every request sent to your API from the Mayhem for API
CLI. These plugins allow you to apply custom authentication, request signing
and other modifications to every request that are not available out of the box.

### [python-rewrite-plugin](python-rewrite-plugin)

This example implements the rewrite plugin service that changes the content-type from application/json to application/grpc-web-text, and base64 encodes the body.

### [python-auth-plugin](python-auth-plugin)

This example implements a service that adds an `Authorization` header to every request.

### [java-auth-plugin](java-auth-plugin)

This example implements the same service as `python-auth-plugin` but written in Java.

### [rust-openid-token-plugin](rust-openid-token-plugin)

A plugin that injects an authorization header retreived from an
OAuth OpenID Connect access token endpoint, implemented in Rust.
This example implements the rewrite plugin adding an `Authorization` header to every request.

### [typescript-rewrite-plugin](typescript-rewrite-plugin)

This example shows how to use typescript as a plugin.

### Response Classify Plugin Examples

[Response Classify plugins](https://app.mayhem.security/docs/api-testing/guides/classify-plugin/) allow you to classify reponses recieved from the server with customer headers, status codes, or other data.

### [python-classify-plugin](python-classify-plugin)

This example implements the classify plugin by parsing the body for grpc-statuses and using them to create HTTP reponses.