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

Build this project using `cargo`. You can install the Rust toolchain
from [rustup.rs](https://rustup.rs/) to get `cargo`. The compiled
binary will be located at `./target/release/ddshark`.


```sh
cargo build --release
```


Specify `-i <INC>` to scan RTPS packets from a network interface. You
may run with `sudo` to grant the permission for packet capture.

```sh
sudo ./target/release/ddshark -i eno1          # Watch an network interface
```


It also supports offline mode. Specify `-f <FILE>` to read packets
from a pre-recorded .pcap file.

```sh
./target/release/ddshark -f packets.pcap  # Read from a .pcap dump
```


## License

It is distributed with a MIT license. Please see the [LICENSE](LICENSE) file.
