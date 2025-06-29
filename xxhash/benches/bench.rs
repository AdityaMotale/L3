use criterion::{
    criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion, Throughput,
};
use rand::{Rng, RngCore, SeedableRng};
use std::{env::consts::ARCH, hash::Hasher as _, iter};
use xxhash::XxHash32;

const BIG_DATA_SIZE: usize = 4 * 1024 * 1024;
const MIN_BIG_DATA_SIZE: usize = 256 * 1024;
const SEED: u64 = 0xc651_4843_1995_363f;

trait CriterionExt {
    fn my_benchmark_group(&mut self, algo: &str, bench: &str) -> BenchmarkGroup<'_, WallTime>;
}

impl CriterionExt for Criterion {
    fn my_benchmark_group(&mut self, algo: &str, bench: &str) -> BenchmarkGroup<'_, WallTime> {
        self.benchmark_group(format!("arch-{ARCH}/algo-{algo}/bench-{bench}"))
    }
}

fn gen_data(length: usize) -> (u64, Vec<u8>) {
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);

    let seed = rng.random();

    let mut data = vec![0; length];
    rng.fill_bytes(&mut data);

    (seed, data)
}

fn half_sizes(max: usize) -> impl Iterator<Item = usize> {
    iter::successors(Some(max), |&v| if v == 1 { None } else { Some(v / 2) })
}

mod xxhash_32 {
    use super::*;

    const TINY_DATA_SIZE: usize = 2;

    fn tiny_data(c: &mut Criterion) {
        let (seed, data) = gen_data(TINY_DATA_SIZE);
        let mut g = c.my_benchmark_group("xxhash32", "tiny_data");

        for size in 0..=data.len() {
            let data = &data[..size];
            g.throughput(Throughput::Bytes(data.len() as _));

            let id = format!("size-{size:02}");
            g.bench_function(id, |b| b.iter(|| XxHash32::oneshot(seed as u32, data)));
        }

        g.finish();
    }

    fn oneshot(c: &mut Criterion) {
        let (seed, data) = gen_data(BIG_DATA_SIZE);
        let mut g = c.my_benchmark_group("xxhash32", "oneshot");

        for size in half_sizes(data.len()).take_while(|&s| s >= MIN_BIG_DATA_SIZE) {
            let data = &data[..size];
            g.throughput(Throughput::Bytes(data.len() as _));

            let id = format!("size-{size:07}");
            g.bench_function(id, |b| b.iter(|| XxHash32::oneshot(seed as u32, data)));
        }

        g.finish();
    }

    fn streaming(c: &mut Criterion) {
        let mut g = c.my_benchmark_group("xxhash32", "streaming");

        let size = 1024 * 1024;
        let (seed, data) = gen_data(size);

        for chunk_size in half_sizes(size) {
            let chunks = data.chunks(chunk_size).collect::<Vec<_>>();

            g.throughput(Throughput::Bytes(size as _));

            let id = format!("size-{size:07}/chunk_size-{chunk_size:02}");
            g.bench_function(id, |b| {
                b.iter(|| {
                    let mut hasher = XxHash32::with_seed(seed as u32);
                    for chunk in &chunks {
                        hasher.write(chunk);
                    }
                    hasher.finish()
                })
            });
        }

        g.finish();
    }

    criterion_group!(benches, tiny_data, oneshot, streaming);
}

criterion_main!(xxhash_32::benches);
