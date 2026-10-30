mod encode;
mod session;

use std::fmt::Write as _;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use wafer_data::packets::{configuration, handshake, login, play};
use wafer_data::{PROTOCOL_VERSION, packet_name};
use wafer_protocol::{Codec, Reader, Writer};
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use session::Session;

const USAGE: &str = include_str!("usage.txt");

struct Options {
    addr: String,
    duration: Duration,
    out: String,
    view_distance: u8,
    name: String,
    wait: Duration,
    walk: bool,
    say: Option<String>,
    dig: Option<u32>,
    script: Vec<String>,
    close: Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Close {
    Fin,
    Reset,
    Hang,
}

fn options() -> anyhow::Result<Options> {
    let mut args = std::env::args().skip(1).peekable();
    let usage = USAGE;
    if args.peek().is_some_and(|arg| arg == "--help" || arg == "-h") {
        print!("{USAGE}");
        std::process::exit(0);
    }
    let mut o = Options {
        addr: args.next().context(usage)?,
        duration: Duration::from_secs(args.next().context(usage)?.parse()?),
        out: args.next().context(usage)?,
        view_distance: 10,
        name: "WaferProbe".into(),
        wait: Duration::ZERO,
        walk: false,
        say: None,
        dig: None,
        script: Vec::new(),
        close: Close::Fin,
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--view" => o.view_distance = value()?.parse()?,
            "--name" => o.name = value()?,
            "--wait" => o.wait = Duration::from_millis(value()?.parse()?),
            "--walk" => o.walk = true,
            "--say" => o.say = Some(value()?),
            "--dig" => o.dig = Some(value()?.parse()?),
            "--script" => o.script = std::fs::read_to_string(value()?)?.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect(),
            "--close" => {
                o.close = match value()?.as_str() {
                    "fin" => Close::Fin,
                    "reset" => Close::Reset,
                    "hang" => Close::Hang,
                    other => bail!("unknown close mode {other}\n{usage}"),
                }
            }
            _ => bail!("unknown option {flag}\n{usage}"),
        }
    }
    Ok(o)
}

struct Recorder {
    start: Instant,
    out: BufWriter<tokio::fs::File>,
    seen: Vec<(String, usize)>,
}

impl Recorder {
    async fn record(&mut self, state: &str, at: Instant, bytes: &[u8]) -> anyhow::Result<(i32, Vec<u8>)> {
        let mut r = Reader::new(bytes);
        let id = r.varint()?;
        let data = bytes[bytes.len() - r.remaining()..].to_vec();
        let name = packet_name(state, "clientbound", id).unwrap_or("unknown");
        let mut hex = String::with_capacity(data.len() * 2);
        data.iter().for_each(|b| write!(hex, "{b:02x}").unwrap());
        let line = serde_json::json!({
            "t_ms": at.duration_since(self.start).as_millis() as u64,
            "state": state,
            "id": id,
            "name": name,
            "len": data.len(),
            "data": hex,
        });
        self.out.write_all(format!("{line}\n").as_bytes()).await?;
        match self.seen.last_mut() {
            Some((last, count)) if last == name => *count += 1,
            _ => self.seen.push((name.to_owned(), 1)),
        }
        Ok((id, data))
    }
}

async fn send<W: AsyncWrite + Unpin>(w: &mut W, codec: Codec, id: i32, body: impl FnOnce(&mut Writer)) -> anyhow::Result<()> {
    let mut p = Writer::new();
    p.varint(id);
    body(&mut p);
    w.write_all(&codec.encode(p.as_slice())).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let o = options()?;
    tokio::time::sleep(o.wait).await;
    let (host, port) = o.addr.rsplit_once(':').context("address must be host:port")?;
    let mut rec = Recorder {
        start: Instant::now(),
        out: BufWriter::new(tokio::fs::File::create(&o.out).await?),
        seen: Vec::new(),
    };
    let mut stream = TcpStream::connect(&o.addr).await?;
    stream.set_nodelay(true)?;
    if o.close == Close::Reset {
        stream.set_linger(Some(Duration::ZERO))?;
    }
    let mut codec = Codec::default();

    send(&mut stream, codec, handshake::serverbound::INTENTION, |w| {
        w.varint(PROTOCOL_VERSION);
        w.string(host);
        w.u16(port.parse().unwrap());
        w.varint(2);
    })
    .await?;
    send(&mut stream, codec, login::serverbound::HELLO, |w| {
        w.string(&o.name);
        w.uuid(0);
    })
    .await?;
    loop {
        let bytes = codec.read(&mut stream).await?;
        let (id, data) = rec.record("login", Instant::now(), &bytes).await?;
        match id {
            login::clientbound::LOGIN_COMPRESSION => codec.threshold = Some(Reader::new(&data).varint()? as usize),
            login::clientbound::LOGIN_FINISHED => break,
            login::clientbound::LOGIN_DISCONNECT => bail!("disconnected during login: {}", Reader::new(&data).string(262144)?),
            _ => {}
        }
    }
    send(&mut stream, codec, login::serverbound::LOGIN_ACKNOWLEDGED, |_| {}).await?;
    send(&mut stream, codec, configuration::serverbound::CLIENT_INFORMATION, |w| {
        w.string("en_us");
        w.u8(o.view_distance);
        w.varint(0);
        w.bool(true);
        w.u8(0x7F);
        w.varint(1);
        w.bool(false);
        w.bool(true);
        w.varint(0);
    })
    .await?;
    loop {
        let bytes = codec.read(&mut stream).await?;
        let (id, data) = rec.record("configuration", Instant::now(), &bytes).await?;
        match id {
            configuration::clientbound::SELECT_KNOWN_PACKS => {
                send(&mut stream, codec, configuration::serverbound::SELECT_KNOWN_PACKS, |w| w.bytes(&data)).await?
            }
            configuration::clientbound::KEEP_ALIVE => {
                send(&mut stream, codec, configuration::serverbound::KEEP_ALIVE, |w| w.bytes(&data)).await?
            }
            configuration::clientbound::PING => send(&mut stream, codec, configuration::serverbound::PONG, |w| w.bytes(&data)).await?,
            configuration::clientbound::FINISH_CONFIGURATION => break,
            configuration::clientbound::DISCONNECT => bail!("disconnected during configuration"),
            _ => {}
        }
    }
    send(&mut stream, codec, configuration::serverbound::FINISH_CONFIGURATION, |_| {}).await?;

    let (mut rd, mut wr) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok(bytes) = codec.read(&mut rd).await {
            if tx.send((Instant::now(), bytes)).is_err() {
                break;
            }
        }
    });

    let deadline = tokio::time::Instant::now() + o.duration;
    let mut ticker = tokio::time::interval(Duration::from_millis(50));
    let mut ticks_loaded = 0u32;
    let mut script = o.script.iter();
    let mut s = Session::new();
    loop {
        tokio::select! {
            packet = rx.recv() => {
                let Some((at, bytes)) = packet else { break };
                let (id, data) = rec.record("play", at, &bytes).await?;
                if let Err(error) = s.on_packet(id, &data) {
                    eprintln!("probe: cannot decode {}: {error:#}", packet_name("play", "clientbound", id).unwrap_or("unknown"));
                }
                match id {
                    play::clientbound::KEEP_ALIVE => send(&mut wr, codec, play::serverbound::KEEP_ALIVE, |w| w.bytes(&data)).await?,
                    play::clientbound::PING => send(&mut wr, codec, play::serverbound::PONG, |w| w.bytes(&data)).await?,
                    play::clientbound::CHUNK_BATCH_FINISHED => {
                        send(&mut wr, codec, play::serverbound::CHUNK_BATCH_RECEIVED, |w| w.f32(64.0)).await?
                    }
                    play::clientbound::PLAYER_POSITION => {
                        let mut r = Reader::new(&data);
                        let teleport = r.varint()?;
                        let pos = (r.f64()?, r.f64()?, r.f64()?);
                        if s.pos.is_none() {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        send(&mut wr, codec, play::serverbound::ACCEPT_TELEPORTATION, |w| {
                            w.varint(teleport);
                            w.f64(pos.0);
                            w.f64(pos.1);
                            w.f64(pos.2);
                            w.f32(0.0);
                            w.f32(0.0);
                        })
                        .await?;
                        if s.pos.is_none() {
                            send(&mut wr, codec, play::serverbound::PLAYER_LOADED, |_| {}).await?;
                        }
                        s.pos = Some(pos);
                    }
                    play::clientbound::DISCONNECT => break,
                    _ => {}
                }
            }
            _ = ticker.tick() => {
                let Some(mut at) = s.pos else { continue };
                ticks_loaded += 1;
                if o.walk {
                    at.0 += 0.2;
                    s.pos = Some(at);
                    send(&mut wr, codec, play::serverbound::MOVE_PLAYER_POS, |w| {
                        w.f64(at.0);
                        w.f64(at.1);
                        w.f64(at.2);
                        w.u8(1);
                    })
                    .await?;
                }
                s.tick_keys();
                s.tick_activity();
                if ticks_loaded == 20 && let Some(command) = o.say.as_ref().and_then(|m| m.strip_prefix('/')) {
                    send(&mut wr, codec, play::serverbound::CHAT_COMMAND, |w| w.string(command)).await?;
                } else if ticks_loaded == 20 && let Some(message) = &o.say {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
                    send(&mut wr, codec, play::serverbound::CHAT, |w| {
                        w.string(message);
                        w.i64(now);
                        w.i64(0);
                        w.bool(false);
                        w.varint(0);
                        w.bytes(&[0, 0, 0]);
                        w.u8(1);
                    })
                    .await?;
                }
                let mut lines = Vec::new();
                if ticks_loaded >= 20 && !s.aborted && !s.script_blocked() && let Some(line) = script.next() {
                    if line.starts_with("suggest ") {
                        lines.push(line.as_str());
                    } else {
                        lines.extend(split_commands(line));
                    }
                }
                for line in lines {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    if let Err(error) = s.command(&words, line) {
                        eprintln!("probe: {line}: {error:#}");
                        s.aborted = true;
                    }
                }
                flush(&mut wr, codec, &mut s).await?;
                if s.riding() {
                    s.tick_riding();
                    flush(&mut wr, codec, &mut s).await?;
                } else {
                    s.send_input_if_changed();
                    flush(&mut wr, codec, &mut s).await?;
                    if o.dig.is_some() {
                        send(&mut wr, codec, play::serverbound::MOVE_PLAYER_ROT, |w| {
                            w.f32(0.0);
                            w.f32(90.0);
                            w.u8(1);
                        })
                        .await?;
                    } else if !o.walk {
                        send(&mut wr, codec, play::serverbound::MOVE_PLAYER_STATUS_ONLY, |w| w.u8(s.on_ground)).await?;
                    }
                    if let Some(dig_ticks) = o.dig {
                        let (x, y, z) = s.pos.unwrap_or(at);
                        let below = (x.floor() as i32, y.floor() as i32 - 1, z.floor() as i32);
                        let action = if ticks_loaded == 20 { Some((0, 1)) } else if ticks_loaded == 20 + dig_ticks { Some((3, 2)) } else { None };
                        if (20..20 + dig_ticks).contains(&ticks_loaded) {
                            send(&mut wr, codec, play::serverbound::PUNCH, |_| {}).await?;
                        }
                        if let Some((status, sequence)) = action {
                            send(&mut wr, codec, play::serverbound::PLAYER_ACTION, |w| {
                                w.varint(status);
                                w.position(below.0, below.1, below.2);
                                w.u8(1);
                                w.varint(sequence);
                            })
                            .await?;
                        }
                    }
                }
                send(&mut wr, codec, play::serverbound::CLIENT_TICK_END, |_| {}).await?;
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }
    rec.out.flush().await?;
    for (name, count) in &rec.seen {
        println!("{count:>5} x {name}");
    }
    if o.close == Close::Hang {
        tokio::time::sleep(Duration::from_secs(120)).await;
    }
    Ok(())
}

fn split_commands(line: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let (mut depth, mut quote, mut start) = (0u32, None, 0);
    let mut chars = line.char_indices();
    while let Some((index, c)) = chars.next() {
        match (c, quote) {
            ('\\', Some(_)) => {
                chars.next();
            }
            (c, Some(q)) if c == q => quote = None,
            (_, Some(_)) => {}
            ('"' | '\'', None) => quote = Some(c),
            ('[' | '{', None) => depth += 1,
            (']' | '}', None) => depth = depth.saturating_sub(1),
            (';', None) if depth == 0 => {
                commands.push(line[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    commands.push(line[start..].trim());
    commands
}

async fn flush<W: AsyncWrite + Unpin>(w: &mut W, codec: Codec, s: &mut Session) -> anyhow::Result<()> {
    for packet in s.outbox.drain(..) {
        w.write_all(&codec.encode(&packet)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::split_commands;

    #[test]
    fn split_commands_splits_on_top_level_semicolons() {
        assert_eq!(split_commands("wait 1; keys forward ;rot 0 0"), ["wait 1", "keys forward", "rot 0 0"]);
    }

    #[test]
    fn split_commands_keeps_nbt_and_quoted_semicolons() {
        let line = "/summon nautilus 0 -60 3 {Owner:[I;1,2,3,4],CustomName:'\"a;b\"'}; wait_new_entity nautilus";
        assert_eq!(split_commands(line), ["/summon nautilus 0 -60 3 {Owner:[I;1,2,3,4],CustomName:'\"a;b\"'}", "wait_new_entity nautilus"]);
    }
}
