use criterion::{
    criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion, Throughput,
};
use std::env::consts::ARCH;
use y3::Y3;

trait CriterionExt {
    fn my_benchmark_group(&mut self, algo: &str, bench: &str) -> BenchmarkGroup<'_, WallTime>;
}

impl CriterionExt for Criterion {
    fn my_benchmark_group(&mut self, algo: &str, bench: &str) -> BenchmarkGroup<'_, WallTime> {
        self.benchmark_group(format!("arch-{ARCH}/algo-{algo}/bench-{bench}"))
    }
}

mod v1 {
    use super::*;

    const FILES: [&str; 2] = ["large.txt", "e_large.txt"];
    const SIZES: [usize; 2] = [3864798, 97447];

    fn file_read(c: &mut Criterion) {
        let mut g = c.my_benchmark_group("y3", "tokenization");

        for (idx, &f) in FILES.iter().enumerate() {
            let path = format!("./ex_files/{f}");
            let mut y3 = Y3::new(&path);
            let size = SIZES[idx];
            let id = format!("iter-{size:08}-{f}");

            g.throughput(Throughput::Bytes(size as _));
            g.bench_function(id, |b| b.iter(|| y3.tokenize()));
        }

        g.finish();
    }

    criterion_group!(bench, file_read);
}

criterion_main!(v1::bench);
