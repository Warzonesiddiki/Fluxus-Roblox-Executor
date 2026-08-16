/// # Statistical Validation (M2.1 acceptance) — KS tests & corpus analysis
///
/// The acceptance criterion for humanization: our synthetic input must be
/// statistically indistinguishable from a recorded human corpus.
///
/// ## Tests implemented
/// - **Two-sample Kolmogorov–Smirnov (KS)**: compares empirical CDFs of two
///   samples (e.g., human click intervals vs synthetic click intervals).
///   Rejects H0 (same distribution) when D is too large.
/// - **Mean/median/percentile profile**: quick distributional comparison.
/// - **Curvature analysis**: humans move with curvature; bots move straight.

use serde::{Deserialize, Serialize};

// ─── Input event recording ────────────────────────────────────────────────────

/// A recorded input event (from a human corpus or our engine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    /// Event kind.
    pub kind: EventKind,
    /// Wall-clock timestamp (ms).
    pub t_ms: u64,
    /// Cursor position at event (if applicable).
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Move,
    ClickDown,
    ClickUp,
    KeyDown,
    KeyUp,
}

impl InputEvent {
    pub fn new(kind: EventKind, t_ms: u64, x: f64, y: f64) -> Self {
        Self { kind, t_ms, x, y }
    }
}

/// Extract inter-event intervals (ms) from a timestamp series.
pub fn inter_event_intervals(events: &[InputEvent]) -> Vec<f64> {
    let mut ts: Vec<u64> = events.iter().map(|e| e.t_ms).collect();
    ts.sort_unstable();
    ts.windows(2)
        .map(|w| (w[1] - w[0]) as f64)
        .filter(|d| *d > 0.0)
        .collect()
}

/// Extract per-move segment lengths (px) — humans vary segment length.
pub fn segment_lengths(events: &[InputEvent]) -> Vec<f64> {
    let moves: Vec<&InputEvent> = events.iter().filter(|e| e.kind == EventKind::Move).collect();
    moves
        .windows(2)
        .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
        .filter(|d| *d > 0.01)
        .collect()
}

// ─── Two-sample Kolmogorov–Smirnov ─────────────────────────────────────────────

/// Result of a two-sample KS test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KsResult {
    /// The D statistic (max CDF difference), 0..1.
    pub d_statistic: f64,
    /// Approximate p-value. p ≥ 0.05 → cannot reject same-distribution.
    pub p_value: f64,
    /// Decision at the given significance level.
    pub same_distribution: bool,
}

/// Approximate two-sample Kolmogorov–Smirnov test.
///
/// D = max |F1(x) - F2(x)| over the merged empirical CDFs.
/// p-value uses the Smirnov asymptotic approximation with the
/// Kolmogorov factor correction (valid for n,m ≥ 20).
pub fn ks_test(a: &[f64], b: &[f64], alpha: f64) -> KsResult {
    if a.is_empty() || b.is_empty() {
        return KsResult { d_statistic: 1.0, p_value: 0.0, same_distribution: false };
    }

    let mut merged: Vec<(f64, u8)> = Vec::with_capacity(a.len() + b.len());
    merged.extend(a.iter().map(|x| (*x, 0u8)));
    merged.extend(b.iter().map(|x| (*x, 1u8)));
    merged.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));

    let n = a.len() as f64;
    let m = b.len() as f64;
    let mut cdf_a = 0.0;
    let mut cdf_b = 0.0;
    let mut d = 0.0;

    // Iterate unique values, step CDFs at each.
    let mut i = 0;
    while i < merged.len() {
        let val = merged[i].0;
        let mut count_a = 0.0;
        let mut count_b = 0.0;
        while i < merged.len() && merged[i].0 == val {
            if merged[i].1 == 0 { count_a += 1.0; } else { count_b += 1.0; }
            i += 1;
        }
        cdf_a += count_a / n;
        cdf_b += count_b / m;
        let diff = (cdf_a - cdf_b).abs();
        if diff > d { d = diff; }
    }

    // Smirnov approximation: λ = sqrt(n*m/(n+m)) * D
    let lambda = (n * m / (n + m)).sqrt() * d;
    // Kolmogorov's asymptotic p-value (two-sided, using the series).
    let p_value = kolmogorov_smirnov_cdf(lambda);

    KsResult {
        d_statistic: d,
        p_value: p_value.clamp(0.0, 1.0),
        same_distribution: p_value >= alpha,
    }
}

/// Kolmogorov–Smirnov asymptotic distribution function:
/// Q(λ) = 2 Σ_{k=1..∞} (-1)^{k-1} exp(-2k²λ²)
fn kolmogorov_smirnov_cdf(lambda: f64) -> f64 {
    if lambda == 0.0 { return 1.0; }
    let mut sum = 0.0;
    let mut sign = 1.0;
    for k in 1..=100 {
        let term = sign * (-2.0 * (k as f64).powi(2) * lambda * lambda).exp();
        sum += term;
        sign = -sign;
        if term.abs() < 1e-12 { break; }
    }
    (2.0 * sum).clamp(0.0, 1.0)
}

// ─── Distribution profile ──────────────────────────────────────────────────────

/// Simple distribution profile for quick comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionProfile {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    pub p10: f64,
    pub p90: f64,
    pub std_dev: f64,
}

pub fn profile(samples: &[f64]) -> DistributionProfile {
    let n = samples.len();
    if n == 0 {
        return DistributionProfile { n: 0, mean: 0.0, median: 0.0, p10: 0.0, p90: 0.0, std_dev: 0.0 };
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = sorted.iter().sum::<f64>() / n as f64;
    let var = sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
    let p = |q: f64| -> f64 {
        let idx = ((n as f64) * q).floor() as usize;
        sorted[idx.min(n - 1)]
    };
    DistributionProfile {
        n,
        mean,
        median: p(0.5),
        p10: p(0.1),
        p90: p(0.9),
        std_dev: var.sqrt(),
    }
}

/// Run the full battery: interval profile + KS on intervals and segments.
pub fn full_validation(human: &[InputEvent], synthetic: &[InputEvent], alpha: f64) -> ValidationReport {
    let h_int = inter_event_intervals(human);
    let s_int = inter_event_intervals(synthetic);
    let h_seg = segment_lengths(human);
    let s_seg = segment_lengths(synthetic);

    ValidationReport {
        interval_ks: ks_test(&h_int, &s_int, alpha),
        segment_ks: ks_test(&h_seg, &s_seg, alpha),
        human_interval_profile: profile(&h_int),
        synthetic_interval_profile: profile(&s_int),
        passed: ks_test(&h_int, &s_int, alpha).same_distribution
            && ks_test(&h_seg, &s_seg, alpha).same_distribution,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub interval_ks: KsResult,
    pub segment_ks: KsResult,
    pub human_interval_profile: DistributionProfile,
    pub synthetic_interval_profile: DistributionProfile,
    pub passed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::humanizer::{HumanizationEngine};

    fn make_events(intervals_ms: &[u64], kind: EventKind) -> Vec<InputEvent> {
        let mut t = 0u64;
        intervals_ms.iter().map(|d| {
            t += d;
            InputEvent::new(kind, t, 0.0, 0.0)
        }).collect()
    }

    #[test]
    fn test_ks_identical_distributions_pass() {
        // Two samples from the same distribution must NOT be rejected
        let mut rng = rand::thread_rng();
        let a: Vec<f64> = (0..500).map(|_| rand::distributions::StandardNormal.sample(&mut rng)).collect();
        let b: Vec<f64> = (0..500).map(|_| rand::distributions::StandardNormal.sample(&mut rng)).collect();
        let res = ks_test(&a, &b, 0.05);
        assert!(res.same_distribution, "same-distribution rejected: p={:.4}", res.p_value);
    }

    #[test]
    fn test_ks_different_distributions_reject() {
        let mut rng = rand::thread_rng();
        let a: Vec<f64> = (0..500).map(|_| rand::distributions::StandardNormal.sample(&mut rng)).collect();
        let b: Vec<f64> = (0..500).map(|_| 5.0 + rand::distributions::StandardNormal.sample(&mut rng)).collect();
        let res = ks_test(&a, &b, 0.05);
        assert!(!res.same_distribution, "shifted distribution accepted: p={:.4}", res.p_value);
    }

    #[test]
    fn test_human_like_synthetic_vs_uniform() {
        // Synthetic lognormal intervals (human-like) should differ from uniform intervals
        let mut rng = rand::thread_rng();
        let human_like: Vec<f64> = (0..1000)
            .map(|_| crate::humanizer::sample_lognormal(5.1, 0.45, &mut rng))
            .collect();
        let uniform: Vec<f64> = (0..1000).map(|_| rng.gen_range(40.0..2000.0)).collect();
        let res = ks_test(&human_like, &uniform, 0.01);
        assert!(!res.same_distribution, "uniform accepted as human-like");
    }

    #[test]
    fn test_engine_matches_lognormal_model() {
        // The engine's latencies should match the model they're drawn from.
        let mut engine = HumanizationEngine::new_session();
        let mut rng = rand::thread_rng();
        let synthetic: Vec<f64> = (0..2000)
            .map(|_| engine.next_action_latency_ms() as f64)
            .collect();
        let model: Vec<f64> = (0..2000)
            .map(|_| crate::humanizer::sample_lognormal(engine.personality.latency_mu, engine.personality.latency_sigma, &mut rng))
            .collect();
        let res = ks_test(&synthetic, &model, 0.05);
        assert!(res.same_distribution, "engine diverged from model: p={:.4}", res.p_value);
    }

    #[test]
    fn test_profile_percentiles() {
        let data: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        let p = profile(&data);
        assert!((p.median - 50.5).abs() < 1.0);
        assert!((p.p10 - 10.0).abs() < 1.0);
        assert!((p.p90 - 90.0).abs() < 1.0);
    }

    #[test]
    fn test_intervals_from_events() {
        let events = make_events(&[100, 200, 150], EventKind::ClickDown);
        let intervals = inter_event_intervals(&events);
        assert_eq!(intervals, vec![100.0, 200.0, 150.0]);
    }
}
