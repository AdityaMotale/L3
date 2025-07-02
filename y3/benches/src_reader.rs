use criterion::{
    criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion, Throughput,
};
use std::env::consts::ARCH;
use y3::SrcReader;

trait CriterionExt {
    fn my_benchmark_group(&mut self, algo: &str, bench: &str) -> BenchmarkGroup<'_, WallTime>;
}

impl CriterionExt for Criterion {
    fn my_benchmark_group(&mut self, algo: &str, bench: &str) -> BenchmarkGroup<'_, WallTime> {
        self.benchmark_group(format!("arch-{ARCH}/algo-{algo}/bench-{bench}"))
    }
}

mod src_reader {
    use super::*;
    use criterion::BenchmarkId;
    use std::{fs::File, hint::black_box, path::PathBuf};

    fn get_file_size(path: &PathBuf) -> usize {
        let file = File::open(path).unwrap();

        file.metadata().unwrap().len() as usize
    }

    fn file_read(c: &mut Criterion) {
        let mut g = c.my_benchmark_group("y3", "src_reader");
        let src_files = std::fs::read_dir("./ex_files").unwrap();

        for f_name in src_files {
            let path = f_name.unwrap().path();
            let id = format!("{:?}", path.file_name().unwrap());
            let size = get_file_size(&path);

            g.throughput(Throughput::Bytes(size as u64));
            g.bench_with_input(BenchmarkId::from_parameter(id), &path, |b, path| {
                b.iter(|| {
                    let mut sr = SrcReader::new(&path).unwrap();

                    while let Some(chunk) = sr.get_chunk() {
                        black_box(chunk);
                    }
                });
            });
        }

        g.finish();
    }

    criterion_group!(bench, file_read);
}

criterion_main!(src_reader::bench);
