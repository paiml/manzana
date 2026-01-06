//! Benchmarks for Afterburner module.
//!
//! These benchmarks verify performance requirements:
//! - F091: Stats query < 1ms latency
//! - F092: No allocations in hot path (where possible)

#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use manzana::afterburner::{AfterburnerStats, ProResCodec};
use std::collections::HashMap;

fn bench_stats_is_active(c: &mut Criterion) {
    let stats = AfterburnerStats {
        streams_active: 10,
        streams_capacity: 23,
        utilization_percent: 45.0,
        throughput_fps: 120.0,
        temperature_celsius: Some(65.0),
        power_watts: Some(25.0),
        codec_breakdown: HashMap::new(),
    };

    c.bench_function("stats_is_active", |b| {
        b.iter(|| black_box(stats.is_active()));
    });
}

fn bench_stats_capacity_used_percent(c: &mut Criterion) {
    let stats = AfterburnerStats {
        streams_active: 10,
        streams_capacity: 23,
        utilization_percent: 45.0,
        throughput_fps: 120.0,
        temperature_celsius: Some(65.0),
        power_watts: Some(25.0),
        codec_breakdown: HashMap::new(),
    };

    c.bench_function("stats_capacity_used_percent", |b| {
        b.iter(|| black_box(stats.capacity_used_percent()));
    });
}

fn bench_stats_is_temperature_safe(c: &mut Criterion) {
    let stats = AfterburnerStats {
        streams_active: 10,
        streams_capacity: 23,
        utilization_percent: 45.0,
        throughput_fps: 120.0,
        temperature_celsius: Some(65.0),
        power_watts: Some(25.0),
        codec_breakdown: HashMap::new(),
    };

    c.bench_function("stats_is_temperature_safe", |b| {
        b.iter(|| black_box(stats.is_temperature_safe()));
    });
}

fn bench_stats_clone(c: &mut Criterion) {
    let stats = AfterburnerStats {
        streams_active: 10,
        streams_capacity: 23,
        utilization_percent: 45.0,
        throughput_fps: 120.0,
        temperature_celsius: Some(65.0),
        power_watts: Some(25.0),
        codec_breakdown: HashMap::new(),
    };

    c.bench_function("stats_clone", |b| {
        b.iter(|| black_box(stats.clone()));
    });
}

fn bench_prores_codec_display(c: &mut Criterion) {
    let codec = ProResCodec::ProRes422HQ;

    c.bench_function("prores_codec_display", |b| {
        b.iter(|| black_box(codec.to_string()));
    });
}

fn bench_stats_default(c: &mut Criterion) {
    c.bench_function("stats_default", |b| {
        b.iter(|| black_box(AfterburnerStats::default()));
    });
}

criterion_group!(
    benches,
    bench_stats_is_active,
    bench_stats_capacity_used_percent,
    bench_stats_is_temperature_safe,
    bench_stats_clone,
    bench_prores_codec_display,
    bench_stats_default,
);

criterion_main!(benches);
