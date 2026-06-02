use criterion::{black_box, criterion_group, criterion_main, Criterion};

use dmart_shared::models::{ApacheIIData, GcsData};
use dmart_shared::scales::*;

fn bench_apache_ii(c: &mut Criterion) {
    let data = ApacheIIData::default();
    c.bench_function("apache_ii_score", |b| {
        b.iter(|| calculate_apache_ii_score(black_box(&data)))
    });
    c.bench_function("apache_ii_breakdown", |b| {
        b.iter(|| apache_ii_breakdown(black_box(&data)))
    });
    c.bench_function("mortality_risk", |b| {
        b.iter(|| mortality_risk(black_box(25)))
    });
}

fn bench_gcs(c: &mut Criterion) {
    let gcs = GcsData::default();
    c.bench_function("gcs_score", |b| {
        b.iter(|| calculate_gcs_score(black_box(&gcs)))
    });
}

fn bench_sofa(c: &mut Criterion) {
    let data = ApacheIIData::default();
    c.bench_function("sofa_breakdown", |b| {
        b.iter(|| sofa_breakdown(black_box(&data)))
    });
}

fn bench_news2(c: &mut Criterion) {
    let data = ApacheIIData::default();
    c.bench_function("news2_breakdown", |b| {
        b.iter(|| news2_breakdown(black_box(&data)))
    });
}

fn bench_saps3(c: &mut Criterion) {
    let data = ApacheIIData::default();
    c.bench_function("saps3_breakdown", |b| {
        b.iter(|| calculate_saps3_breakdown(black_box(&data)))
    });
    c.bench_function("saps3_score", |b| {
        b.iter(|| calculate_saps_iii_score(black_box(&data)))
    });
}

criterion_group!(
    benches,
    bench_apache_ii,
    bench_gcs,
    bench_sofa,
    bench_news2,
    bench_saps3,
);
criterion_main!(benches);
