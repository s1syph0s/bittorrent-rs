use core::panic;
use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

// fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
//     // If encoded_value starts with a digit, it's a number
//     if encoded_value.chars().next().unwrap().is_ascii_digit() {
//         // Example: "5:hello" -> "hello"
//         let colon_index = encoded_value.find(':').unwrap();
//         let number_string = &encoded_value[..colon_index];
//         let number = number_string.parse::<usize>().unwrap();
//         let string = &encoded_value[colon_index + 1..colon_index + 1 + number];
//         serde_json::Value::String(string.to_string())
//     } else {
//         panic!("Unhandled encoded value: {}", encoded_value)
//     }
// }

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> (serde_json::Value, &str) {
    match encoded_value.chars().next() {
        Some('i') => {
            if let Some((n, rest)) =
                encoded_value
                    .split_at(1)
                    .1
                    .split_once('e')
                    .and_then(|(nums, rest)| {
                        let n = nums.parse::<i64>().ok()?;
                        Some((n, rest))
                    })
            {
                return (n.into(), rest);
            }
        }
        Some('l') => {
            let mut v = Vec::new();
            let mut rest = encoded_value.split_at(1).1;
            while !rest.starts_with('e') {
                let (val, new_rest) = decode_bencoded_value(rest);
                v.push(val);
                rest = new_rest;
            }
            return (v.into(), &rest[1..]);
        }
        }
        Some('0'..='9') => {
            if let Some((len, rest)) = encoded_value.split_once(':') {
                if let Ok(len) = len.parse::<usize>() {
                    return (rest[..len].to_string().into(), &rest[len..]);
                }
            }
        }
        _ => {}
    }
    panic!("Unhandled encoded value: {}", encoded_value);
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        // You can use print statements as follows for debugging, they'll be visible when running tests.
        // eprintln!("Logs from your program will appear here!");

        // Uncomment this block to pass the first stage
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.0);
    } else {
        println!("unknown command: {}", args[1])
    }
}
