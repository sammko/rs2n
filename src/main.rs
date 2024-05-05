use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use argh::FromArgs;
use log::{info, warn};
use tokio::{
    net::{TcpListener, TcpStream},
    time::sleep,
};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

#[derive(FromArgs)]
/// Make a serial port available via TCP.
struct Args {
    /// address to listen on
    #[argh(
        option,
        short = 'l',
        default = "SocketAddr::from(([0, 0, 0, 0], 20108))"
    )]
    listen: SocketAddr,

    /// serial port
    #[argh(positional)]
    device: String,

    /// serial port baud rate
    #[argh(option, short = 'b', default = "115200")]
    baud_rate: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();

    let args: Args = argh::from_env();

    let listener = TcpListener::bind(args.listen).await?;

    loop {
        info!("Listening on {}", listener.local_addr()?);

        let tcp = match listener.accept().await {
            Ok((stream, peer_addr)) => {
                info!("Accepted connection from {peer_addr}");
                stream
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted => continue,
            Err(e) => match e.raw_os_error() {
                Some(libc::EMFILE | libc::ENFILE) => {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
                _ => Err(e).context("Fatal accept() error")?,
            },
        };

        let serial = match tokio_serial::new(&args.device, args.baud_rate).open_native_async() {
            Ok(serial) => serial,
            Err(e) => {
                warn!("Failed to open serial port: {e}");
                continue;
            }
        };

        match run(tcp, serial).await {
            Ok(_) => info!("session done"),
            Err(e) => warn!("failed: {e}"),
        }
    }
}

async fn run(tcp: TcpStream, serial: SerialStream) -> Result<()> {
    let (mut tcp_rd, mut tcp_wr) = tokio::io::split(tcp);
    let (mut ser_rd, mut ser_wr) = tokio::io::split(serial);

    tokio::select! {
        _ = tokio::io::copy(&mut tcp_rd, &mut ser_wr) => {},
        _ = tokio::io::copy(&mut ser_rd, &mut tcp_wr) => {}
    }

    return Ok(());
}
