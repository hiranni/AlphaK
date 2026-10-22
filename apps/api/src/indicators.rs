use crate::models::Bar;

fn sma(values: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if n == 0 {
        return out;
    }
    let mut sum = 0.0;
    for i in 0..values.len() {
        sum += values[i];
        if i >= n {
            sum -= values[i - n];
        }
        if i + 1 >= n {
            out[i] = Some(sum / n as f64);
        }
    }
    out
}

fn ema(values: &[f64], n: usize) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    let alpha = 2.0 / (n as f64 + 1.0);
    let mut out = Vec::with_capacity(values.len());
    let mut prev = values[0];
    out.push(prev);
    for v in values.iter().skip(1) {
        prev = alpha * v + (1.0 - alpha) * prev;
        out.push(prev);
    }
    out
}

fn rsi(values: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if values.len() <= n {
        return out;
    }
    let mut gain = 0.0;
    let mut loss = 0.0;
    for i in 1..=n {
        let diff = values[i] - values[i - 1];
        if diff >= 0.0 {
            gain += diff;
        } else {
            loss -= diff;
        }
    }
    let mut avg_gain = gain / n as f64;
    let mut avg_loss = loss / n as f64;
    out[n] = Some(if avg_loss == 0.0 {
        100.0
    } else {
        100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
    });
    for i in (n + 1)..values.len() {
        let diff = values[i] - values[i - 1];
        let (g, l) = if diff >= 0.0 {
            (diff, 0.0)
        } else {
            (0.0, -diff)
        };
        avg_gain = (avg_gain * (n as f64 - 1.0) + g) / n as f64;
        avg_loss = (avg_loss * (n as f64 - 1.0) + l) / n as f64;
        out[i] = Some(if avg_loss == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
        });
    }
    out
}

type KdjSeries = (Vec<Option<f64>>, Vec<Option<f64>>, Vec<Option<f64>>);

fn kdj(bars: &[Bar], n: usize) -> KdjSeries {
    let mut k_out = vec![None; bars.len()];
    let mut d_out = vec![None; bars.len()];
    let mut j_out = vec![None; bars.len()];
    let mut k = 50.0;
    let mut d = 50.0;
    for i in 0..bars.len() {
        if i + 1 < n {
            continue;
        }
        let window = &bars[i + 1 - n..=i];
        let hi = window.iter().map(|b| b.high).fold(f64::MIN, f64::max);
        let lo = window.iter().map(|b| b.low).fold(f64::MAX, f64::min);
        let rsv = if hi == lo {
            50.0
        } else {
            (bars[i].close - lo) / (hi - lo) * 100.0
        };
        k = (2.0 * k + rsv) / 3.0;
        d = (2.0 * d + k) / 3.0;
        let j = 3.0 * k - 2.0 * d;
        k_out[i] = Some(k);
        d_out[i] = Some(d);
        j_out[i] = Some(j);
    }
    (k_out, d_out, j_out)
}

pub fn apply(bars: &mut [Bar]) {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();
    let vol_ma5 = sma(&volumes, 5);
    let vol_ma10 = sma(&volumes, 10);
    let ma5 = sma(&closes, 5);
    let ma10 = sma(&closes, 10);
    let ma20 = sma(&closes, 20);
    let ma60 = sma(&closes, 60);
    let e12 = ema(&closes, 12);
    let boll_mid = sma(&closes, 20);
    let mut boll_upper = vec![None; closes.len()];
    let mut boll_lower = vec![None; closes.len()];
    for i in 0..closes.len() {
        if let Some(mid) = boll_mid[i] {
            let window = &closes[i + 1 - 20..=i];
            let variance = window.iter().map(|v| (v - mid).powi(2)).sum::<f64>() / 20.0;
            let sd = variance.sqrt();
            boll_upper[i] = Some(mid + 2.0 * sd);
            boll_lower[i] = Some(mid - 2.0 * sd);
        }
    }
    let e26 = ema(&closes, 26);
    let dif: Vec<f64> = e12.iter().zip(e26.iter()).map(|(a, b)| a - b).collect();
    let dea = ema(&dif, 9);
    let rsi14 = rsi(&closes, 14);
    let (k, d, j) = kdj(bars, 9);

    for i in 0..bars.len() {
        bars[i].ma5 = ma5[i];
        bars[i].ma10 = ma10[i];
        bars[i].ma20 = ma20[i];
        bars[i].ma60 = ma60[i];
        bars[i].vol_ma5 = vol_ma5[i];
        bars[i].vol_ma10 = vol_ma10[i];
        bars[i].ema12 = Some(e12[i]);
        bars[i].boll_upper = boll_upper[i];
        bars[i].boll_mid = boll_mid[i];
        bars[i].boll_lower = boll_lower[i];
        if i >= 26 {
            bars[i].dif = Some(dif[i]);
            bars[i].dea = Some(dea[i]);
            bars[i].macd = Some((dif[i] - dea[i]) * 2.0);
        }
        bars[i].k = k[i];
        bars[i].d = d[i];
        bars[i].j = j[i];
        bars[i].rsi = rsi14[i];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(close: f64, high: f64, low: f64) -> Bar {
        Bar {
            date: "2026-01-01".into(),
            open: close,
            high,
            low,
            close,
            volume: 1.0,
            amount: 1.0,
            ma5: None,
            ma10: None,
            ma20: None,
            ma60: None,
            vol_ma5: None,
            vol_ma10: None,
            ema12: None,
            boll_upper: None,
            boll_mid: None,
            boll_lower: None,
            dif: None,
            dea: None,
            macd: None,
            k: None,
            d: None,
            j: None,
            rsi: None,
        }
    }

    #[test]
    fn sma_has_warmup() {
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let out = sma(&vals, 3);
        assert_eq!(out[1], None);
        assert_eq!(out[2], Some(2.0));
        assert_eq!(out[4], Some(4.0));
    }

    #[test]
    fn ema_matches_known_sequence() {
        let out = ema(&[1.0, 2.0, 3.0, 4.0, 5.0], 3);
        let expected = [1.0, 1.5, 2.25, 3.125, 4.0625];
        for (actual, expected) in out.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn constant_series_has_flat_boll_macd_kdj() {
        let mut bars: Vec<Bar> = (0..30).map(|_| bar(10.0, 11.0, 9.0)).collect();
        apply(&mut bars);
        let last = &bars[29];
        assert!((last.boll_mid.unwrap() - 10.0).abs() < 1e-12);
        assert!((last.boll_upper.unwrap() - 10.0).abs() < 1e-12);
        assert!((last.boll_lower.unwrap() - 10.0).abs() < 1e-12);
        assert!((last.dif.unwrap()).abs() < 1e-12);
        assert!((last.dea.unwrap()).abs() < 1e-12);
        assert!((last.macd.unwrap()).abs() < 1e-12);
        assert!((last.k.unwrap() - 50.0).abs() < 1e-12);
        assert!((last.d.unwrap() - 50.0).abs() < 1e-12);
        assert!((last.j.unwrap() - 50.0).abs() < 1e-12);
    }

    #[test]
    fn rsi_extremes_are_100_and_0() {
        let up: Vec<f64> = (1..=20).map(|i| i as f64).collect();
        let down: Vec<f64> = (1..=20).rev().map(|i| i as f64).collect();
        assert!((rsi(&up, 14)[19].unwrap() - 100.0).abs() < 1e-12);
        assert!((rsi(&down, 14)[19].unwrap() - 0.0).abs() < 1e-12);
    }

    #[test]
    fn indicators_are_populated() {
        let mut bars: Vec<Bar> = (1..=80)
            .map(|i| bar(i as f64, i as f64 + 1.0, i as f64 - 1.0))
            .collect();
        apply(&mut bars);
        assert!(bars[79].ma20.is_some());
        assert!((bars[79].vol_ma5.unwrap() - 1.0).abs() < 1e-12);
        assert!((bars[79].vol_ma10.unwrap() - 1.0).abs() < 1e-12);
        assert!(bars[79].dif.is_some());
        assert!(bars[79].rsi.is_some());
        assert!(bars[79].k.is_some());
    }
}
