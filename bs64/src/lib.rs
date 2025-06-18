#![feature(portable_simd)]

pub fn encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();

    for chunk in input.chunks_exact(3) {
        let c = (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | chunk[2] as u32;

        let b1 = ((c >> 18) & 0x3F) as usize;
        let b2 = ((c >> 12) & 0x3F) as usize;
        let b3 = ((c >> 6) & 0x3F) as usize;
        let b4 = (c & 0x3F) as usize;

        output.push(TABLE[b1] as char);
        output.push(TABLE[b2] as char);
        output.push(TABLE[b3] as char);
        output.push(TABLE[b4] as char);
    }

    let rem = input.chunks_exact(3).remainder();

    match rem.len() {
        0 => { /* Do nothing */ }
        1 => {
            let c = (rem[0] as u32) << 16;
            let b1 = ((c >> 18) & 0x3F) as usize;
            let b2 = ((c >> 12) & 0x3F) as usize;

            output.push(TABLE[b1] as char);
            output.push(TABLE[b2] as char);
            output.push_str("==");
        }
        2 => {
            let c = (rem[0] as u32) << 16 | (rem[1] as u32) << 8;

            let b1 = ((c >> 18) & 0x3F) as usize;
            let b2 = ((c >> 12) & 0x3F) as usize;
            let b3 = ((c >> 6) & 0x3F) as usize;

            output.push(TABLE[b1] as char);
            output.push(TABLE[b2] as char);
            output.push(TABLE[b3] as char);
            output.push_str("=");
        }
        _ => unreachable!(),
    }

    output
}

pub fn decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: [u8; 256] = {
        let mut table = [255u8; 256];
        let bytes = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        let mut i = 0;

        while i < bytes.len() {
            table[bytes[i] as usize] = i as u8;
            i += 1;
        }

        table
    };

    let len = input.len();
    return None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(String::from("TXkgU3RyaW5n"), encode("My String".as_bytes()));
    }
}
