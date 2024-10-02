use peers::Peers;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct TrackerRequest {
    pub peer_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    pub port: u16,
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<Event>,
    pub compact: u8,
}

impl TrackerRequest {
    pub fn http_query_params(&self) -> String {
        serde_urlencoded::to_string(self).unwrap()
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Event {
    Started,
    Completed,
    Stopped,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackerResponse {
    #[serde(flatten)]
    pub resp_type: ResponseType,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ResponseType {
    Ok {
        #[serde(rename = "interval")]
        interval: usize,
        #[serde(rename = "peers")]
        peers: Peers,
    },
    Err {
        #[serde(rename = "failure reason")]
        fail_reason: String,
    },
}

pub mod peers {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use serde::de::{Deserialize, Visitor};

    #[derive(Debug, Clone)]
    pub struct Peers(pub Vec<SocketAddrV4>);
    struct PeersVisitor;

    impl<'de> Visitor<'de> for PeersVisitor {
        type Value = Peers;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("6 bytes, the first 4 is the addr and the last 2 is the port")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v.len() % 6 != 0 {
                return Err(E::custom(format!("length is {}", v.len())));
            }
            Ok(Peers(
                v.chunks_exact(6)
                    .map(|slice_6| {
                        SocketAddrV4::new(
                            Ipv4Addr::new(slice_6[0], slice_6[1], slice_6[2], slice_6[3]),
                            u16::from_be_bytes([slice_6[4], slice_6[5]]),
                        )
                    })
                    .collect(),
            ))
        }
    }

    impl<'de> Deserialize<'de> for Peers {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_bytes(PeersVisitor)
        }
    }
}
