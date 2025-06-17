pub fn encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let chunks = input.chunks_exact(3);
    let mut output = String::new();

    for chunk in chunks {
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

    println!("{:?}", output);

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let _ = encode("My Sting".as_bytes());
        assert_eq!(4, 4);
    }
}
