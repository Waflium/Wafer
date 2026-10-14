<div align="center">

<img src=".github/logo.png" alt="Wafer" width="360">

<h3>A high-performance Minecraft server, written in Rust</h3>

</div>

<br>

Wafer is a drop-in replacement for the Minecraft Java Edition 26.3 dedicated server. It behaves exactly like vanilla, and runs as fast as the hardware allows.

- **Faithful.** Gameplay, world generation, saved files and network traffic match vanilla one to one.
- **Fast.** World generation, lighting, saving, networking and compression run in parallel.
- **Drop-in.** Uses the same worlds, settings and player data as vanilla.

## Getting started

```sh
cargo xtask vanilla
cargo build --release
```

Run `target/release/wafer` in a folder with an accepted `eula.txt`.

## License

[PolyForm Noncommercial License 1.0.0](LICENSE)

<br>

<div align="center">
<sub>Not affiliated with Mojang Studios or Microsoft.</sub>
</div>
