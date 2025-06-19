use kvdb::{Result, Store};

fn main() -> Result<()> {
    let mut db = Store::open("/tmp/mini-dbdir")?;
    db.set(b"hello", b"world")?;

    println!("{:?}", db.get(b"hello")?);
    println!("{:?}", db.get(b"nonexistent")?);

    db.remove(b"hello")?;
    println!("{:?}", db.get(b"hello")?);

    println!("{}", db.iter().count());

    for i in 0..100_000u32 {
        db.set(&i.to_le_bytes(), &(i * 2).to_le_bytes())?;
    }

    println!("{}", db.iter().count());

    Ok(())
}
