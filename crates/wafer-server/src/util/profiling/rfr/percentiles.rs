const DEFAULT_INDEXES: [i32; 4] = [50, 75, 90, 99];

pub fn evaluate(dataset: &[f64]) -> Vec<(i32, f64)> {
    if dataset.is_empty() {
        return Vec::new();
    }
    let mut sorted = dataset.to_vec();
    let has_nan = sorted.iter().any(|value| value.is_nan());
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mut percentiles: Vec<(i32, f64)> = DEFAULT_INDEXES
        .iter()
        .map(|&index| {
            if has_nan {
                return (index, f64::NAN);
            }
            let numerator = index as i64 * (sorted.len() as i64 - 1);
            let quotient = (numerator / 100) as usize;
            let remainder = numerator - quotient as i64 * 100;
            let value = if remainder == 0 { sorted[quotient] } else { interpolate(sorted[quotient], sorted[quotient + 1], remainder as f64, 100.0) };
            (index, value)
        })
        .collect();
    percentiles.sort_by(|a, b| b.0.cmp(&a.0));
    percentiles
}

fn interpolate(lower: f64, upper: f64, remainder: f64, scale: f64) -> f64 {
    if lower == f64::NEG_INFINITY {
        return if upper == f64::INFINITY { f64::NAN } else { f64::NEG_INFINITY };
    }
    if upper == f64::INFINITY {
        return f64::INFINITY;
    }
    lower + (upper - lower) * remainder / scale
}

pub fn evaluate_longs(dataset: &[i64]) -> Vec<(i32, f64)> {
    evaluate(&dataset.iter().map(|&value| value as f64).collect::<Vec<_>>())
}

pub fn evaluate_ints(dataset: &[i32]) -> Vec<(i32, f64)> {
    evaluate(&dataset.iter().map(|&value| value as f64).collect::<Vec<_>>())
}
