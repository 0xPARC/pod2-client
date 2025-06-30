# POD2 Client Tools and Experiments

This repo contains experimental client tools and applications for [POD2](https://github.com/0xPARC/pod2).

## Components

### ./server

The `server` provides a basic HTTP API for interacting with POD2 and a local POD collection. The local POD collection is managed in a SQLite database.

### ./solver

The `solver` provides a Datalog engine for POD Request queries. It is written in Rust and is used by the `server` to provide certain APIs.

### ./web/packages/pod2js

The `pod2js` library provides TypeScript types for the core POD2 data structures. These are derived from the JSON Schema for the POD2 types, which is created in Rust using the `schemars` crate. The library also provides validation functions for serialized `SignedPod` and `MainPod` JSON data.

### ./web/apps/playground-client

The `playground-client` provides a user interface for interacting with the `server`. Specifically it allows users to import and export PODs, to create new SignedPods via a form-based UI, and to create new MainPods using POD Requests.
