#![allow(dead_code)]
use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use core::panic;
use futures::{SinkExt, StreamExt};
use nanoid::nanoid;
use rand::seq::SliceRandom;
use rand::thread_rng;
use sha1::{Digest, Sha1};
use std::{env, net::SocketAddrV4, path::PathBuf};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use bittorrent_rs::{
    peer::{Handshake, Message, MessageCodec, MessageTag, Piece, Request},
    torrent::{self, Torrent},
    tracker::{ResponseType, TrackerRequest, TrackerResponse},
};

const BLOCK_MAX: u64 = 1 << 14;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Decode {
        value: String,
    },
    Info {
        torrent: PathBuf,
    },
    Peers {
        torrent: PathBuf,
    },
    Handshake {
        torrent: PathBuf,
        peer: String,
    },
    #[command(name = "download_piece")]
    DownloadPiece {
        #[arg(short)]
        output: PathBuf,
        torrent: PathBuf,
        piece: usize,
    },
    Download {
        #[arg(short)]
        output: PathBuf,
        torrent: PathBuf,
    },
}

fn urlencoded(info_hash: &[u8; 20]) -> String {
    let mut encoded = String::with_capacity(3 * info_hash.len());
    for &byte in info_hash {
        encoded.push('%');
        encoded.push_str(&hex::encode([byte]));
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
                bittorrent_rs::tracker::ResponseType::Ok { interval: _, peers } => {
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

            let handshake_b = handshake.as_mut_bytes();
            peer.write_all(handshake_b).await?;
            peer.read_exact(handshake_b).await?;

            assert_eq!(&handshake.msg, b"BitTorrent protocol");
            println!("Peer ID: {}", hex::encode(handshake.peer_id));
        }
        Command::DownloadPiece {
            output,
            torrent,
            piece: piece_i,
        } => {
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

            let mut rng = thread_rng();
            let peers_vec = match tracker_resp.resp_type {
                bittorrent_rs::tracker::ResponseType::Ok { interval: _, peers } => peers.0,
                bittorrent_rs::tracker::ResponseType::Err { fail_reason } => {
                    bail!("{}", fail_reason);
                }
            };
            let peer = peers_vec
                .choose(&mut rng)
                .expect("peers should be returned");

            let peer = TcpStream::connect(peer).await.context("connect to peer")?;

            let all_blocks = download_piece(
                peer,
                t,
                piece_i,
                &request.peer_id.into_bytes().try_into().unwrap(),
            )
            .await?;

            tokio::fs::write(&output, all_blocks)
                .await
                .context("write out downloaded piece")?;

            println!("Piece {piece_i} downloaded to {}.", output.display());
        }
        Command::Download { output, torrent } => {
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

            let mut rng = thread_rng();
            let peers_vec = match tracker_resp.resp_type {
                bittorrent_rs::tracker::ResponseType::Ok { interval: _, peers } => peers.0,
                bittorrent_rs::tracker::ResponseType::Err { fail_reason } => {
                    bail!("{}", fail_reason);
                }
            };

            let peer = peers_vec
                .choose(&mut rng)
                .expect("peers should be returned");

            let peer = TcpStream::connect(peer).await.context("connect to peer")?;
        }
    }
    Ok(())
}

async fn download_piece(
    mut peer: TcpStream,
    t: Torrent,
    piece_i: usize,
    peer_id: &[u8; 20],
) -> anyhow::Result<Vec<u8>> {
    let length = if let torrent::Keys::SingleFile { length } = t.info.keys {
        length
    } else {
        panic!("key should be either singlefile or multifile!");
    };

    // Handshake
    let mut handshake = Handshake::new(t.info_hash(), *peer_id);
    let handshake_b = handshake.as_mut_bytes();
    peer.write_all(handshake_b).await?;
    peer.read_exact(handshake_b).await?;

    assert_eq!(&handshake.msg, b"BitTorrent protocol");

    let mut peer = tokio_util::codec::Framed::new(peer, MessageCodec);

    // Wait for bitfield msg
    let bitfield = peer
        .next()
        .await
        .expect("peer always sends a bitfields")
        .context("peer msg was invalid")?;

    assert_eq!(bitfield.tag, MessageTag::Bitfield);

    // Send interested msg
    peer.send(Message {
        tag: MessageTag::Interested,
        payload: Vec::new(),
    })
    .await
    .context("send interested msg")?;

    // Wait for unchoke msg
    let unchoke = peer
        .next()
        .await
        .expect("peer always sends an unchoke")
        .context("peer msg was invalid")?;

    assert_eq!(unchoke.tag, MessageTag::Unchoke);

    let piece_hash = &t.info.pieces.0[piece_i];

    // last piece can be smaller than the plength
    let piece_size = if piece_i == t.info.pieces.0.len() - 1 {
        let md = length % t.info.plength;
        if md == 0 {
            t.info.plength
        } else {
            md
        }
    } else {
        t.info.plength
    };
    let nblocks = piece_size.div_ceil(BLOCK_MAX);

    let mut all_blocks = Vec::with_capacity(piece_size as usize);
    for block in 0..nblocks {
        let block_size = if block == nblocks - 1 {
            let md = piece_size % BLOCK_MAX;
            if md == 0 {
                BLOCK_MAX
            } else {
                md
            }
        } else {
            BLOCK_MAX
        };
        let mut request = Request::new(
            piece_i as u32,
            (block * BLOCK_MAX) as u32,
            block_size as u32,
        );

        let request_bytes = Vec::from(request.as_bytes_mut());

        peer.send(Message {
            tag: MessageTag::Request,

            payload: request_bytes,
        })
        .await
        .with_context(|| format!("send request for block {block}"))?;

        let piece = peer
            .next()
            .await
            .expect("peer always sends a piece")
            .context("peer message was invalid")?;

        assert_eq!(piece.tag, MessageTag::Piece);

        let piece = Piece::ref_from_bytes(&piece.payload[..])
            .expect("always get all Piece response fields from peer");

        assert_eq!(piece.index() as usize, piece_i);
        assert_eq!(piece.begin() as u64, block * BLOCK_MAX);
        assert_eq!(piece.block().len(), block_size as usize);

        all_blocks.extend(piece.block());
    }
    assert_eq!(all_blocks.len(), piece_size as usize);

    let mut hasher = Sha1::new();
    hasher.update(&all_blocks);

    let hash: [u8; 20] = hasher.finalize().into();

    assert_eq!(&hash, piece_hash);

    Ok(all_blocks)
}
