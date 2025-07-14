# IC API Boundary Node Access Logs Client

A sample WebSocket client that demonstrates how to subscribe to API boundary node access logs of the Internet Computer.

## Overview

This tool connects to all API boundary nodes of the Internet Computer simultaneously and streams real-time logs for the specified canister.

## Usage

```bash
# Run with a canister ID
cargo run -- --canister-id <CANISTER_ID>

# Example
cargo run -- --canister-id qoctq-giaaa-aaaaa-aaaea-cai
```

### Command Line Options

- `-c, --canister-id <CANISTER_ID>`: The canister ID to monitor logs for (required)
- `-h, --help`: Show help information

## License

This project is licensed under the [Apache License 2.0](LICENSE).

## Contributing

This repository does not accept external contributions at this time.
