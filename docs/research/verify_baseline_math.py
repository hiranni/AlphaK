"""Constructed arithmetic examples only; no market data, fitting or backtest."""
import json
import math
from pathlib import Path


def percentile_midrank(history, value):
    if not history:
        return None
    return 100 * (sum(x < value for x in history)
                  + 0.5 * sum(x == value for x in history)) / len(history)


def score(values, weights):
    coverage = sum(w for x, w in zip(values, weights) if x is not None)
    if any(x is None for x in values):
        return {"total": None, "coverage": coverage}
    contributions = [x * w for x, w in zip(values, weights)]
    return {"total": math.fsum(contributions), "coverage": coverage,
            "contributions": contributions}


def main():
    weights = [.45, .35, .20]
    result = score([80, 60, 70], weights)
    assert math.isclose(sum(weights), 1)
    assert math.isclose(result["total"], 71.0)
    assert math.isclose(math.fsum(result["contributions"]), result["total"])
    missing = score([80, None, 70], weights)
    assert missing["total"] is None and math.isclose(missing["coverage"], .65)
    relative = sum(w * (s - 50) for s, w in zip([80, 60, 70], weights))
    assert math.isclose(relative, result["total"] - 50)

    breadth = (60 - 30) / (60 + 30 + 10)
    assert math.isclose(breadth, .3)
    assert percentile_midrank([1, 2, 2, 3], 2) == 50
    assert percentile_midrank([], 2) is None
    # Two equally weighted opposing event opinions: neutral mean, high disagreement.
    opinions = [-1, 1]
    text_mean = sum(opinions) / 2
    disagreement = sum((x - text_mean) ** 2 for x in opinions) / 2
    assert text_mean == 0 and disagreement == 1
    # With d fixed, marginal breadth effect gamma_b + eta*d can change sign.
    effect_low = .01 - .02 * 0
    effect_high = .01 - .02 * 1
    assert effect_low > 0 and effect_high < 0
    # PSD 2x2 sample covariance; shrink off-diagonal toward zero.
    a, b, c, delta = .04, .09, .05, .5
    shrunk_c = (1 - delta) * c
    det = a * b - shrunk_c ** 2
    assert a > 0 and b > 0 and det >= 0
    risk_variance = .5 ** 2 * a + .5 ** 2 * b + 2 * .5 * .5 * shrunk_c
    assert risk_variance >= 0
    # Decision-contract sizing examples: illustrative fee and 100-share unit only.
    equity, count, price, stop_distance = 100000, 8, 20, .08
    base_value = equity * .95 / count
    risk_value = equity * .005 / stop_distance
    budget = min(base_value, equity * .20, risk_value)
    quantity = int(budget // (100 * price)) * 100
    while quantity and quantity * price + 5 > budget:
        quantity -= 100
    assert base_value == 11875 and risk_value == 6250
    assert quantity == 300 and quantity * price + 5 == 6005
    assert math.isclose(quantity * price * stop_distance, 480)
    # Apply common scale before rounding; no allocation of leftover cash.
    raw_weights = [.0625] * 8
    alpha = min(1, .03 / sum(w * stop_distance for w in raw_weights))
    rounded = [int(equity * w * alpha // (price * 100)) * 100 for w in raw_weights]
    aggregate_risk = sum(q * price * stop_distance for q in rounded) / equity
    assert math.isclose(alpha, .75) and aggregate_risk <= .03
    # Sell enough legal units so the result is no greater than half.
    locked_target = 500 / 2
    sell_qty = math.ceil((500 - locked_target) / 100) * 100
    assert sell_qty == 300 and 500 - sell_qty == 200
    report = {"type": "constructed_arithmetic_only", "market_data_used": False,
              "empirical_validation": False, "checks_passed": True,
              "decision_contract_examples": {"buy_quantity": quantity, "buy_cost": 6005,
                 "planned_loss": 480, "joint_scale": alpha,
                 "rounded_portfolio_risk": aggregate_risk, "half_reduction_sell": sell_qty},
              "quality_example": result, "missing_example": missing,
              "relative_reference_contribution": relative,
              "breadth_example": breadth, "midrank_tie_example": 50,
              "opposing_text_example": {"mean": text_mean, "disagreement": disagreement},
              "conditional_breadth_effect_example": [effect_low, effect_high],
              "covariance_example": {"delta": delta, "determinant": det,
                                     "portfolio_variance": risk_variance}}
    target = Path(__file__).with_name('baseline-math-check.json')
    target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
