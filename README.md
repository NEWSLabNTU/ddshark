# ddshark

Live monitoring tool for Cyclone DDS. It is a work from NEWSLAB,
National Taiwan University.

**Authors**

- Hsiang-Jui Lin (2023)
- Taiyou Kuo (2023)


## Get Started

Download the source code using git.

```bash
git clone https://github.com/jerry73204/ddshark.git
cd ddshark
git submodule update --init --recursive
```

Build this project using `cargo`. You may install the Rust toolchain
from [rustup.rs](https://rustup.rs/) to get `cargo`.


```bash
cargo build            # debug build, or
cargo build --release  # release build
```

The compiled binary will be located at `./target/debug/ddshark` or
`./target/release/ddshark`, depending on your compilation profile.

```bash
./target/debug/ddshark -i eno1            # Watch an network interface
./target/release/ddshark -f packets.pcap  # Read from a .pcap dump
```

## License

It is distributed with a MIT license. Please see the [LICENSE](LICENSE) file.
