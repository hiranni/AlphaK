//! AK-State: purged historical analogue learning and prequential research simulation.
//! Pure model outputs never enter order tables or Agent decision channels.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
const H: usize = 5;
const COST: f64 = 0.001; // Research stress assumption per side, not a broker tariff.
const START: usize = 150;
const FEATURES: [&str; 8] = [
    "5日动量",
    "20日动量",
    "均线偏离",
    "20日波动",
    "区间位置",
    "相对成交量",
    "60日回撤",
    "池内情绪",
];
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Bar {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub source: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Stock {
    pub symbol: String,
    pub name: String,
    pub bars: Vec<Bar>,
}
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
struct Params {
    window: usize,
    k: usize,
    caution: f64,
}
#[derive(Clone, Serialize)]
struct Prediction {
    expected: f64,
    downside: f64,
    uncertainty: f64,
    edge: f64,
    weight: f64,
    support: usize,
    params: Params,
    analogues: Vec<Value>,
    drivers: Vec<Value>,
    validation_score: f64,
}
fn avg(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len().max(1) as f64
}
fn sd(x: &[f64]) -> f64 {
    let a = avg(x);
    avg(&x.iter().map(|x| (x - a).powi(2)).collect::<Vec<_>>()).sqrt()
}
fn quantile(x: &[f64], q: f64) -> f64 {
    let mut x = x.to_vec();
    x.sort_by(f64::total_cmp);
    x[((x.len() - 1) as f64 * q).floor() as usize]
}
fn features(b: &[Bar], i: usize, breadth: &BTreeMap<String, f64>) -> [f64; 8] {
    let c = b[i].close;
    let returns: Vec<_> = b[i - 20..=i]
        .windows(2)
        .map(|w| w[1].close / w[0].close - 1.)
        .collect();
    let vol = sd(&returns).max(0.003);
    let ma = avg(&b[i - 19..=i].iter().map(|b| b.close).collect::<Vec<_>>());
    let high = b[i - 19..=i].iter().map(|b| b.high).fold(0., f64::max);
    let low = b[i - 19..=i]
        .iter()
        .map(|b| b.low)
        .fold(f64::INFINITY, f64::min);
    let peak = b[i - 59..=i].iter().map(|b| b.close).fold(0., f64::max);
    let vm = avg(&b[i - 20..i].iter().map(|b| b.volume).collect::<Vec<_>>());
    [
        (c / b[i - 5].close - 1.) / (vol * 5f64.sqrt()),
        (c / b[i - 20].close - 1.) / (vol * 20f64.sqrt()),
        (c / ma - 1.) / vol,
        vol,
        (c - low) / (high - low).max(0.0001),
        ((b[i].volume + 1.) / (vm + 1.)).ln(),
        c / peak - 1.,
        *breadth.get(&b[i].date).unwrap_or(&0.5),
    ]
}
struct Sample {
    i: usize,
    x: [f64; 8],
    y: f64,
}
fn samples(b: &[Bar], breadth: &BTreeMap<String, f64>) -> Vec<Sample> {
    (60..b.len().saturating_sub(H))
        .filter(|i| b[i + 1].open > 0.)
        .map(|i| Sample {
            i,
            x: features(b, i, breadth),
            y: b[i + H].close / b[i + 1].open - 1.,
        })
        .collect()
}
fn infer(
    b: &[Bar],
    samples: &[Sample],
    i: usize,
    p: Params,
    breadth: &BTreeMap<String, f64>,
    cap: f64,
) -> Option<Prediction> {
    // A label must have fully matured BEFORE the prediction close. No future scaler fitting.
    let train: Vec<_> = samples
        .iter()
        .filter(|s| s.i + H < i && s.i + p.window >= i)
        .collect();
    if train.len() < 40 || p.k > train.len() {
        return None;
    }
    let x = features(b, i, breadth);
    let means: [f64; 8] =
        std::array::from_fn(|j| avg(&train.iter().map(|s| s.x[j]).collect::<Vec<_>>()));
    let scales: [f64; 8] =
        std::array::from_fn(|j| sd(&train.iter().map(|s| s.x[j]).collect::<Vec<_>>()).max(0.00001));
    let mut near: Vec<_> = train
        .iter()
        .map(|s| {
            let distance = (0..8)
                .map(|j| ((s.x[j] - x[j]) / scales[j]).clamp(-5., 5.).powi(2))
                .sum::<f64>();
            (*s, distance)
        })
        .collect();
    near.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.i.cmp(&b.0.i)));
    near.truncate(p.k);
    let ys: Vec<_> = near.iter().map(|(s, _)| s.y).collect();
    let expected = avg(&ys);
    let downside = (-quantile(&ys, 0.1)).max(0.02); // 2% floor prevents zero-risk division.
    let uncertainty = sd(&ys) / (p.k as f64 / H as f64).max(1.).sqrt(); // Overlapping labels: conservative effective sample count.
    let edge = expected - 2. * COST - p.caution * uncertainty;
    let strength = (edge / downside).clamp(0., 1.);
    let weight = if edge > 0. {
        cap.min(0.005 / downside).min(cap * strength)
    } else {
        0.
    };
    let analogues=near.iter().take(5).map(|(s,d)|json!({"date":b[s.i].date,"label_available":b[s.i+H].date,"gross_return":s.y,"distance":d})).collect();
    let drivers=(0..8).map(|j|json!({"name":FEATURES[j],"value":x[j],"training_mean":means[j],"z":(x[j]-means[j])/scales[j]})).collect();
    Some(Prediction {
        expected,
        downside,
        uncertainty,
        edge,
        weight,
        support: train.len(),
        params: p,
        analogues,
        drivers,
        validation_score: 0.,
    })
}
fn candidates() -> Vec<Params> {
    let mut v = vec![];
    for window in [80, 120] {
        for k in [8, 16, 24] {
            for caution in [0., 0.5, 1.] {
                v.push(Params { window, k, caution })
            }
        }
    }
    v
}
fn learn(
    b: &[Bar],
    samples: &[Sample],
    i: usize,
    breadth: &BTreeMap<String, f64>,
    cap: f64,
) -> Option<Prediction> {
    let mut best: Option<(f64, Params)> = None;
    for p in candidates() {
        let mut rewards = vec![];
        for v in ((i.saturating_sub(40)).max(110)..i.saturating_sub(H)).step_by(H) {
            if let Some(pred) = infer(b, samples, v, p, breadth, cap) {
                let y = b[v + H].close / b[v + 1].open - 1.;
                rewards.push(pred.weight * (y - 2. * COST));
            }
        }
        if rewards.len() < 4 {
            continue;
        }
        let score = avg(&rewards) - 0.5 * sd(&rewards);
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, p));
        }
    }
    let (score, p) = best?;
    let mut pred = infer(b, samples, i, p, breadth, cap)?;
    pred.validation_score = score;
    // Cash is an explicit candidate. A negative validation score cannot force a position.
    if score <= 0. {
        pred.weight = 0.;
    }
    Some(pred)
}
fn metrics(nav: &[(String, f64)], closed: usize, fees: f64) -> Value {
    if nav.len() < 2 {
        return json!({"return":null,"max_drawdown":null,"sharpe":null,"closed_trades":closed,"cost":fees});
    }
    let mut peak: f64 = 1.;
    let mut dd: f64 = 0.;
    let returns: Vec<_> = nav.windows(2).map(|p| p[1].1 / p[0].1 - 1.).collect();
    for (_, v) in nav {
        peak = peak.max(*v);
        dd = dd.max(1. - v / peak);
    }
    let s = sd(&returns);
    json!({"return":nav.last().unwrap().1/nav[0].1-1.,"max_drawdown":dd,"sharpe":if s>0.{Some(avg(&returns)/s*252f64.sqrt())}else{None},"closed_trades":closed,"cost":fees,"days":nav.len()})
}
fn simulate(stock: &Stock, breadth: &BTreeMap<String, f64>, n: usize) -> Value {
    let b = &stock.bars;
    if b.len() < START + 21 {
        return json!({"symbol":stock.symbol,"name":stock.name,"status":"insufficient_data","bars":b.len(),"required":START+21});
    }
    let data = samples(b, breadth);
    let cap = 0.2f64.min(0.8 / n as f64);
    let capital = 100000.;
    let mut cash = capital;
    let mut qty = 0.;
    let mut basis = 0.;
    let mut entered = 0;
    let mut pending: Option<(f64, f64, String)> = None;
    let mut nav = vec![];
    let mut trades = vec![];
    let mut fits = vec![];
    let mut fees = 0.;
    let mut closed = 0;
    let mut rejection = vec![];
    for i in START..b.len() {
        if let Some((weight, stop, reason)) = pending.take() {
            let equity = cash + qty * b[i].open;
            let target = (equity * weight / b[i].open / 100.).floor() * 100.;
            let delta = target - qty;
            let ret = b[i].open / b[i - 1].close - 1.;
            let limit = if stock.symbol.starts_with("300") || stock.symbol.starts_with("688") {
                0.195
            } else {
                0.095
            };
            let locked = delta > 0. && ret >= limit || delta < 0. && ret <= -limit;
            let unavailable = b[i].volume <= 0. || b[i].open <= 0.;
            if locked || unavailable {
                rejection.push(json!({"date":b[i].date,"reason":"停牌/开盘涨跌停保守拒绝","target_weight":weight}));
            } else if delta > 0. {
                let cost = (delta * b[i].open * COST).max(5.);
                if delta * b[i].open + cost <= cash {
                    cash -= delta * b[i].open + cost;
                    qty += delta;
                    basis = b[i].open;
                    entered = i;
                    fees += cost;
                    trades.push(json!({"date":b[i].date,"side":"buy","quantity":delta,"price":b[i].open,"cost":cost,"reason":reason,"stop_distance":stop}));
                }
            } else if delta < 0. && i > entered {
                let cost = ((-delta) * b[i].open * COST).max(5.);
                cash += (-delta) * b[i].open - cost;
                qty += delta;
                fees += cost;
                if qty == 0. {
                    closed += 1;
                }
                trades.push(json!({"date":b[i].date,"side":"sell","quantity":-delta,"price":b[i].open,"cost":cost,"reason":reason}));
            }
        }
        let equity = cash + qty * b[i].close;
        nav.push((b[i].date.clone(), equity / capital));
        if qty > 0. && (i >= entered + H || b[i].close / basis - 1. <= -0.08) {
            pending = Some((0., 0.08, "持有5日到期或收盘止损8%".into()));
        } else if qty == 0. && (i - START) % H == 0 {
            if let Some(pred) = learn(b, &data, i, breadth, cap) {
                fits.push(json!({"date":b[i].date,"params":pred.params,"validation_score":pred.validation_score,"expected":pred.expected,"weight":pred.weight}));
                if pred.weight > 0. {
                    pending = Some((
                        pred.weight,
                        pred.downside,
                        "历史类似状态净优势通过，下一开盘尝试".into(),
                    ));
                }
            }
        }
    }
    let latest = learn(b, &data, b.len() - 1, breadth, cap);
    let current_weight = qty * b.last().unwrap().close / (cash + qty * b.last().unwrap().close);
    let last = b.last().unwrap();
    let decision=latest.as_ref().map(|p|{let desired=if qty>0.&&(b.len()-1>=entered+H||last.close/basis-1.<=-0.08){0.}else if qty>0.{current_weight}else{p.weight};json!({"action":if desired>current_weight+0.001{"buy_candidate"}else if desired+0.001<current_weight{"exit_candidate"}else{"wait"},"target_weight":desired,"expected_5d_return":p.expected,"net_edge":p.edge,"historical_downside":p.downside,"uncertainty":p.uncertainty,"params":p.params,"training_samples":p.support,"validation_score":p.validation_score,"analogues":p.analogues,"features":p.drivers,"note":"研究组合口径的下一开盘目标，不是已成交或真实账户建议；缺少严格PIT/公司行动/历史ST规则，禁止自动部署"})});
    let benchmarks: Vec<_> = b[START..]
        .iter()
        .map(|x| (x.date.clone(), 1. + cap * (x.close / b[START].close - 1.)))
        .collect();
    json!({"symbol":stock.symbol,"name":stock.name,"status":"exploratory","as_of":last.date,"bars":b.len(),"decision":decision,"metrics":metrics(&nav,closed,fees),"nav":nav,"baseline":benchmarks,"trades":trades,"rejections":rejection,"fits":fits,"current_weight":current_weight,"cash":cash,"shares":qty})
}
pub fn research(stocks: &[Stock]) -> Value {
    let mut bydate: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for stock in stocks {
        for pair in stock.bars.windows(2) {
            let p = bydate.entry(pair[1].date.clone()).or_default();
            p.0 += usize::from(pair[1].close > pair[0].close);
            p.1 += 1;
        }
    }
    // Only complete date-aligned pools qualify. Missing breadth is a model-blocking input.
    let breadth: BTreeMap<String, f64> = bydate
        .into_iter()
        .filter(|(_, (_, count))| *count == stocks.len())
        .map(|(d, (up, n))| (d, up as f64 / n as f64))
        .collect();
    if stocks.is_empty() || stocks.len() > 8 {
        return json!({"status":"invalid_universe","stocks":[]});
    }
    let aligned = stocks.iter().all(|s| {
        s.bars.len() > START + 20
            && s.bars
                .iter()
                .skip(60)
                .all(|b| breadth.contains_key(&b.date))
    });
    if !aligned {
        return json!({"status":"insufficient_data","reason":"每只至少171根有效日线且60日预热后池内日期完整对齐；缺失不补情绪0.5","stocks":stocks.iter().map(|s|json!({"symbol":s.symbol,"name":s.name,"bars":s.bars.len()})).collect::<Vec<_>>()});
    }
    let reports: Vec<_> = stocks
        .iter()
        .map(|s| simulate(s, &breadth, stocks.len()))
        .collect();
    let mut dates: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for r in &reports {
        if let Some(nav) = r["nav"].as_array() {
            for p in nav {
                dates
                    .entry(p[0].as_str().unwrap().to_string())
                    .or_default()
                    .push(p[1].as_f64().unwrap());
            }
        }
    }
    let nav: Vec<_> = dates
        .into_iter()
        .filter(|(_, v)| v.len() == stocks.len())
        .map(|(d, v)| (d, 1. + v.iter().map(|v| v - 1.).sum::<f64>()))
        .collect();
    let closed = reports
        .iter()
        .filter_map(|r| r["metrics"]["closed_trades"].as_u64())
        .sum::<u64>() as usize;
    let costs = reports
        .iter()
        .filter_map(|r| r["metrics"]["cost"].as_f64())
        .sum::<f64>();
    json!({"model":"AK-State-v1","status":"exploratory","selection":"18组邻域/窗口/谨慎系数，过去验证选参，5日标签严格成熟隔离，每次空仓决策滚动重选","horizon":H,"features":FEATURES,"metrics":metrics(&nav,closed,costs),"nav":nav,"stocks":reports,"limitations":["当前样本池与当前修订历史，非严格PIT或未见最终测试集","未复权，除权及历史ST/上市规则不完整；模拟成交为保守近似","池内上涨占比是盘面情绪代理，非舆情/全市场情绪","每边10bps且最低5元是研究压力假设，不是已核验实际费率","近邻标签重叠，独立样本量有限；当前验证不构成盈利证明","不下真实或模拟账户订单，不自动晋升，不自动调松风险上限"]})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Stock {
        Stock {
            symbol: "002463.SZ".into(),
            name: "fixture".into(),
            bars: (0..245)
                .map(|i| {
                    let price = 20. + i as f64 * 0.03 + (i as f64 / 4.).sin() * 0.2;
                    Bar {
                        date: format!("2025-{:03}", i),
                        open: price,
                        high: price + 0.5,
                        low: price - 0.5,
                        close: price + 0.1,
                        volume: 10000. + (i % 7) as f64 * 100.,
                        source: "test".into(),
                    }
                })
                .collect(),
        }
    }
    #[test]
    fn purged_prediction_ignores_future() {
        let a = fixture();
        let mut b = a.clone();
        for x in b.bars.iter_mut().skip(191) {
            x.close *= 10.;
            x.open *= 10.;
        }
        let breadth = BTreeMap::new();
        let p = Params {
            window: 120,
            k: 16,
            caution: 0.5,
        };
        let x = infer(&a.bars, &samples(&a.bars, &breadth), 190, p, &breadth, 0.2).unwrap();
        let y = infer(&b.bars, &samples(&b.bars, &breadth), 190, p, &breadth, 0.2).unwrap();
        assert_eq!(x.expected, y.expected);
        assert!(x
            .analogues
            .iter()
            .all(|v| v["label_available"].as_str().unwrap() < a.bars[190].date.as_str()));
    }
    #[test]
    fn research_is_deterministic_and_scoped() {
        let s = fixture();
        let a = research(&[s.clone()]);
        assert_eq!(a, research(&[s]));
        assert_eq!(a["status"], "exploratory");
        let nav = a["nav"].as_array().unwrap();
        assert!(!nav.is_empty());
        assert!(nav.iter().all(|p| p[1].as_f64().unwrap() > 0.));
        assert_eq!(research(&[])["status"], "invalid_universe");
    }
}
