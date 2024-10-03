#![allow(dead_code)]
use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use core::panic;
use nanoid::nanoid;
use sha1::{Digest, Sha1};
use std::{env, net::SocketAddrV4, path::PathBuf};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use bittorrent_rs::{
    peer::Handshake,
    torrent::{self, Torrent},
    tracker::{TrackerRequest, TrackerResponse},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Decode { value: String },
    Info { torrent: PathBuf },
    Peers { torrent: PathBuf },
    Handshake { torrent: PathBuf, peer: String },
}

fn urlencoded(info_hash: &[u8; 20]) -> String {
    let mut encoded = String::with_capacity(3 * info_hash.len());
    for &byte in info_hash {
        encoded.push('%');
        encoded.push_str(&hex::encode(&[byte]));
    }
    encoded
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Decode { value } => {
            let v: serde_bencode::value::Value = serde_bencode::from_str(&value)?;
            println!("{:?}", v);
        }
        Command::Info { torrent } => {
            let torrent_f = std::fs::read(torrent).context("read torrent file")?;
            let t: Torrent = serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;

            println!("Tracker URL: {}", t.announce);
            if let torrent::Keys::SingleFile { length } = t.info.keys {
                println!("Length: {length}");
            } else {
                todo!();
            }
            let info_bencoded = serde_bencode::to_bytes(&t.info).context("re-encode info")?;
            println!("{:?}", t);
            let mut hasher = Sha1::new();
            hasher.update(&info_bencoded);
            let info_hash = hasher.finalize();
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", t.info.plength);
            println!("Piece Hashes:");
            for hash in t.info.pieces.0 {
                println!("\t{}", hex::encode(hash));
            }
        }
        Command::Peers { torrent } => {
            let torrent_f = std::fs::read(torrent).context("read torrent file")?;
            let t: Torrent = serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;

            let length = if let torrent::Keys::SingleFile { length } = t.info.keys {
                length
            } else {
                panic!("key should be either singlefile or multifile!");
            };

            let request = TrackerRequest {
                // info_hash: t.info_hash().into(),
                // peer_id: Uuid::new_v4().into(),
                peer_id: nanoid!(20),
                ip: None,
                port: 6969,
                uploaded: 0,
                downloaded: 0,
                left: length,
                event: None,
                compact: 1,
            };
            t.info_hash();

            let tracker_url = format!(
                "{}?{}&info_hash={}",
                t.announce,
                request.http_query_params(),
                urlencoded(&t.info_hash())
            );
            let response = reqwest::get(tracker_url)
                .await
                .context("tracker url response")?;
            let response = response.bytes().await.context("get response bytes")?;

            let tracker_resp: TrackerResponse =
                serde_bencode::from_bytes(&response).context("deserialize response struct")?;
            match tracker_resp.resp_type {
                bittorrent_rs::tracker::ResponseType::Ok { interval, peers } => {
                    peers.0.iter().for_each(|x| println!("{}", x));
                }
                bittorrent_rs::tracker::ResponseType::Err { fail_reason } => {
                    bail!("{}", fail_reason);
                }
            }
        }
        Command::Handshake { torrent, peer } => {
            let torrent_f = std::fs::read(torrent).context("read torrent file")?;
            let t: Torrent = serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;

            let info_hash = t.info_hash();
            let peer_id = nanoid!(20).into_bytes().try_into().unwrap();
            let mut handshake = Handshake::new(info_hash, peer_id);

            let peer = peer.parse::<SocketAddrV4>().context("parsing peer")?;
            let mut peer = TcpStream::connect(peer).await?;

            let handshake_b =
                &mut handshake as *mut Handshake as *mut [u8; std::mem::size_of::<Handshake>()];
            let handshake_b: &mut [u8; std::mem::size_of::<Handshake>()] =
                unsafe { &mut *handshake_b };
            peer.write_all(handshake_b).await?;
            peer.read_exact(handshake_b).await?;

            assert_eq!(&handshake.msg, b"BitTorrent protocol");
            println!("Peer ID: {}", hex::encode(&handshake.peer_id));
        }
    }
    Ok(())
}
