# Artefacta - Manage artefact downloads and patched upgrades

> artefacta, Latin, "the artefacts"

This is a small tool used to package/upload/download/extract software builds.

## Concepts and Features

- All commands operate on an index which is built by listing current builds.
- Builds are stored in both a local and remote store.
- Supports creating and using binary patches using [bidiff]
- Builds and patches are mirrored locally
- Builds and patches are compressed using [zstd]

[bidiff]: https://github.com/divvun/bidiff
[zstd]: https://github.com/facebook/zstd

## Usage

Run `artefacta [command] --help` to discover CLI options.

### Environment variables

- `ARTEFACTA_LOCAL_STORE`: Path to local store (on file system)
- `ARTEFACTA_REMOTE_STORE`: Path to remote store (on file system or S3)
- `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`: Used for authorizing S3 requests
- `ARTEFACTA_COMPRESSION_LEVEL`: Overwrite default compression level used when packaging builds
- `RUST_LOG`: Enable logging beyond what `--verbose` can do.
  See the [`env_logger` docs] for details on the syntax.
  
[`env_logger` docs]: https://docs.rs/env_logger/0.7.1/env_logger/#enabling-logging

### Notes

- Locally, a `current` symlink points at the currently used version (which might or might not be latest one).
- S3 URIs should be formatted like `s3://my-bucket.ams3.digitaloceanspaces.com/test`

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
