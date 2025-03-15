# deno-axum-nuxt

Example of running Tokio proxy in front of an Axum server and Deno Nuxt server.

The tokio proxy routes connections based on whether the request is HTTP and the
path starts with "/api". If the request is HTTP and the path starts with "/api",
it is routed to the Axum server. Otherwise, it is routed to the Deno Nuxt
server.

There is an issue with keep-alive connections when using the tokio proxy. When a
connection is keep-alive, the tokio proxy will not route subsequent requests on
the same connection to the Deno Nuxt server. This is likely due to the Axum
server turning on keep-alive connections by default. If each request is sent
over a new connection, the issue should not occur. Alas, this is not implemented
just yet.
