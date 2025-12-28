# Carbon Cache

Carbon Cache is a high performance cache written in Rust providing multiple strategies for eviction like ttl, size, overflows to Disk giving you the best of both the worlds.

Carbon Cache runs as a HTTP Server allowing you to directly connect to the server via Postman or any HTTP Client library. Carbon aims to support multiple interfaces for connecting to it like TCP & QUIC in the future.

_Carbon supports Basic Auth for Admin tasks._

---

To start Carbon Server:

```bash
cargo run --bin carbon-server --release
```

The command above, does multiple things:

-   Lauches TCP Server on port: 9090
-   Launches HTTP Server on port: 8090
-   Serves Admin UI on path /admin/ui/

Command to build Admin UI from Carbon Http Server:

```bash
dx build --package carbon-admin-ui --release --verbose
```

> This command builds a release version of the Admin and publishes it to the public folder and Carbon HTTP server is configured to serve the public content from there including html, js and wasm.

Command to run Admin independently:

```bash
 dx serve -p carbon-admin-ui
```

> When running independently, make sure to configure the CARBON_ALLOWED_ORIGINS environment variable correctly

---

**_To stop the carbon server, press ctrl+c_**
