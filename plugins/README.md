# Plugin Examples

Request Rewrite plugins are [currently in pre-release](https://mayhem4api.forallsecure.com/docs/rewrite.html).
The examples here may change as the feature evolves.

### [python-auth-plugin](python-auth-plugin)

This example implements a service that adds an `Authorization` header to every request.

### [java-auth-plugin](java-auth-plugin)

This example implements the same service as `python-auth-plugin` but written in Java.

### [rust-openid-token-plugin](rust-openid-token-plugin)

A plugin that injects an authorization header retreived from an
OAuth OpenID Connect access token endpoint, implemented in Rust.
