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

    const NO_FILE: usize = 2;
    const BYTES_SIZE: usize = 3864798;

    fn file_read(c: &mut Criterion) {
        let mut g = c.my_benchmark_group("y3", "file_read");

        for i in 0..NO_FILE {
            let mut y3 = Y3::new("dict.txt");
            g.throughput(Throughput::Bytes(BYTES_SIZE as _));

            let id = format!("v1/iter-{i:02}");
            g.bench_function(id, |b| b.iter(|| y3.tokenize()));
        }

        g.finish();
    }

    criterion_group!(bench, file_read);
}

criterion_main!(v1::bench);
