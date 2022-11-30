# Perimetr

A system to securly handle digital heritage. Provides one or more layers of encryption that need to be unlocked by one or more human or digital secret keepers. The latter are handled by a Dead Man's Switch service that checks sources for signed timestamps of the decendent.

This was developed as a Proof-Of-Concept for my master thesis at the [Leipzig University of Applied Sciences](https://www.htwk-leipzig.de/en/htwk-leipzig/).

## Provided tools

This repo provides three binaries, which must be defined when one runs `cargo build`.

### `perimetr`

A helper tool to create perimetr layers and split secrets into shares for multiple secret keepers using Feldman verifiable secret sharing.

```
$ cargo run --bin perimetr -- --help
Usage: perimetr [COMMAND]

Commands:
  split    Split a secret into shares and store metadata in the metadata-dir.
  combine  Combine shares into a secret with the provided metadata-file
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help information
```

Layer metadata files provide instructions for the server on how to decrypt layers, an example is [`examples/example.layer.yml`](examples/example.layer.yml). Files should to be stored next to the layer file and encrypted in the same way, f.e. like this:
```
$ echo "secret heritage" | gpg -r bob@example.com --encrypt \
-o bob.gpg
$ echo "public heritage" > public.txt
$ tar caf b0bb162f-7db3-43ea-aca3-f91884133740.tar.zst \
bob.gpg public.txt
$ echo "vsss-secret" | gpg --encrypt --symmetric \
--passphrase-fd 0 \
--batch b0bb162f-7db3-43ea-aca3-f91884133740.tar.zst
$ shred -fu b0bb162f-7db3-43ea-aca3-f91884133740.tar.zst \
bob.gpg public.txt
$ ls
b0bb162f-7db3-43ea-aca3-f91884133740.layer.yml
b0bb162f-7db3-43ea-aca3-f91884133740.tar.zst.gpg
```

### `perimetr-server`

Webservice that accepts VSSS shares for perimetr layers and decrypts them when enough shares are received.

![diagram](.images/perimetr.drawio.svg)

```
$ cargo run --bin perimetr-server -- --help
Usage: perimetr-server [OPTIONS]

Options:
  -p, --layer-path <layer-path>      Path to layer files [default: .]
  -s, --layer-suffix <layer-suffix>  Suffix of layer files [default: .layer.yml]
  -d, --database-url <database-url>  PostgreSQL database URL [default: postgres://postgres:postgres@localhost:5432/postgres]
  -b, --bind-host <bind-host>        Host to bind to [default: 127.0.0.1:8080]
  -h, --help                         Print help information
```

Needs a postgresql database, even for development:
```
podman run --rm --name perimetr-pg -p 5432:5432 -e "POSTGRES_PASSWORD=postgres" docker.io/library/postgres:latest
```

![webservice](.images/webservice.png)

### `perimetr-dms`

Service that checks endpoints for signed timestamps and executes commands when threshold are reached.

![diagram](.images/dms.drawio.svg)

```
$ cargo run --bin perimetr-dms -- --help
Usage: perimetr-dms [OPTIONS]

Options:
  -c, --config <config>  Path to config file [default: dms.yml]
  -h, --help             Print help information
```

An example configuration file is [`examples/dms-configuration.yml`](examples/dms-configuration.yml).

A [simple script](scripts/dms-sign.sh) can be used to put signed timestamps on a webserver.

## TODO

### Store metadata in the database

While it is useful to store the metadata next to the encrypted layers it is inefficent to use the filesystem as a database for them. They should be cached in the PostgreSQL database.

## References

1. [How to share a secret, Shamir, A. Nov, 1979](https://dl.acm.org/doi/pdf/10.1145/359168.359176)
2. [A Practical Scheme for Non-interactive Verifiable Secret Sharing, Feldman, P. 1987](https://www.cs.umd.edu/~gasarch/TOPICS/secretsharing/feldmanVSS.pdf)
3. [Verifiable Secret Sharing Schemes, implemented by matt9j and others](https://github.com/matt9j/vsss-rs)