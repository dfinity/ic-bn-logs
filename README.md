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

## Important Notes

This demo illustrates how to connect to the API boundary nodes and stream access logs. It fetches the list of API boundary nodes once at startup, but this list can change over time (typically every few weeks or months).

A production-grade system should:

- Periodically refresh the API boundary node list
- Automatically connect to newly added nodes
- Handle removal of obsolete nodes

Use this code as a starting point for building a more resilient log streaming solution.

## License

This project is licensed under the [Apache License 2.0](LICENSE).

## Contributing

This repository does not accept external contributions at this time.
