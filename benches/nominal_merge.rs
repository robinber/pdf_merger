use criterion::{criterion_group, criterion_main, Criterion};
use pdf_merger_lig::merge_pdfs;

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("PDF Merge");
    group.warm_up_time(std::time::Duration::new(5, 0)); // Temps de chauffage de 5 secondes
    group.measurement_time(std::time::Duration::new(10, 0)); // Temps de mesure de 10 secondes
    group.plot_config(
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic),
    );

    group.bench_function("nominal_merge", |b| b.iter(|| load_and_merge_documents()));

    group.finish();
}

fn load_and_merge_documents() -> Result<lopdf::Document, lopdf::Error> {
    let paths = Vec::from([
        "tests/pdf_samples/in/pdf1.pdf",
        "tests/pdf_samples/in/pdf2.pdf",
        "tests/pdf_samples/in/pdf3.pdf",
        "tests/pdf_samples/in/pdf4.pdf",
    ]);

    merge_pdfs(&paths)
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
