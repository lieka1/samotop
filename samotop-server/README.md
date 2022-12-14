[![Build Status](https://gitlab.com/BrightOpen/Samotop/badges/develop/pipeline.svg)](https://gitlab.com/BrightOpen/Samotop/commits/master)

# samotop-server 1.2.2

You can run your own privacy focussed, resource efficient mail server. [Samotop docker image](https://hub.docker.com/r/brightopen/samotop) is available for your convenience.

## Status

### General

- [x] Tiny docker image - only contains statically compiled samotop and openssl, no OS clutter.

### Common (MDA/MTA/MSA)

- [x] The server will receive mail and write it to a given maildir folder. Another program can pick the folder and process it further.
- [x] STARTTLS can be configured if you provide a cert and identity file.

### Mail delivery agent (MDA)

- [ ] Encryption at rest
- [ ] Accounts
- [ ] LMTP
- [ ] Sockets

### Mail transfer agent (MTA)

- [ ] Mail relaying
- [ ] Antispam features:
  - [x] SPF - refuse mail with failing SPF check
  - [ ] Greylisting

### Mail submission agent (MSA)

- [ ] Authentication

## Installation

- Using cargo:
   ```bash
   cargo install samotop-server
   ```
- Using docker:
   ```bash
   docker pull brightopen/samotop
   ```

## Usage

- locally, run `samotop-server --help` for command-line reference.
- in docker, run `docker run --rm -ti samotop`

Both should produce a usage information not too different from this:

    samotop 1.2.0

    USAGE:
        samotop-server [FLAGS] [OPTIONS] --cert-file <cert file path> --identity-file <identity file path>

    FLAGS:
        -h, --help       Prints help information
            --no-tls     Disable TLS suport
        -V, --version    Prints version information

    OPTIONS:
        -n, --name <SMTP service name>              Use the given name in SMTP greetings, or if absent, use hostname
        -b, --base-dir <base dir path>              What is the base dir for other relative paths? [default: .]
        -c, --cert-file <cert file path>            Use this cert file for TLS. Disabled with --no-tls. If a relative path
                                                    is given, it will be relative to base-dir
            --banner_delay <delay>                  Should we enforce prudent banner deleay? Delay is in miliseconds
        -i, --identity-file <identity file path>    Use this identity file for TLS. Disabled with --no-tls. If a relative
                                                    path is given, it will be relative to base-dir
        -m, --mail-dir <mail dir path>              Where to store incoming mail? If a relative path is given, it will be
                                                    relative to base-dir [default: inmail]
        -p, --port <port>...                        SMTP server address:port, such as 127.0.0.1:25 or localhost:12345. The
                                                    option can be set multiple times and the server will start on all given
                                                    ports. If no ports are given, the default is to start on localhost:25
            --command_timeout <timeout>             Should we enforce prudent command timeout? Timeout is in miliseconds

## TLS

You can run these openssl commands in docker as well.
This will run an openssl with the current folder mounted under /data and that is also the work dir:
```rust
docker run --rm -ti -v "$PWD:/data/" -w "/data/" --entrypoint openssl samotop help
```

Generate a cert and ID with openssl:
```bash
openssl req -new -newkey rsa:4096 -x509 -sha256 -days 365 -nodes -out Samotop.crt -keyout Samotop.key
```

Test STARTTLS:
```bash
openssl s_client -connect localhost:25 -starttls smtp
```

Debug with STARTTLS:
```bash
openssl s_client -connect localhost:25 -debug -starttls smtp
```

### Other useful hints for TLS

For native-tls, you'd convert to pfx:
```bash
openssl pkcs12 -export -out Samotop.pfx -inkey Samotop.key -in Samotop.crt
```

Extracting pub key from cert:
```bash
openssl x509 -pubkey -noout -in Samotop.crt  > Samotop.pem
```


## License
MIT OR Apache-2.0

### Contribution
Unless you explicitly state otherwise, any contribution submitted for inclusion in samotop projects by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.
