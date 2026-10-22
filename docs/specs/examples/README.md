# Examples · 业务有效性说明

`agent-decision.example.json` 是**成功样例**，但“通过 JSON Schema”不等于“通过业务校验”。示例使用 1,000,000 元分配资金、200,000 元单笔上限、20% 单票上限；订单合计 196,510 元，风险说明中列出对应检查。

| 文件 | Schema | 业务预期 | 说明 |
| --- | --- | --- | --- |
| `agent-response.example.json` | 通过 | 接受（作为模型语义响应） | 仅含模型字段；运行时注入 envelope 后再生成完整决策 |
| `agent-decision.example.json` | 通过 | 接受 | 池内标的、手数合法、现金与限额满足 |
| 反例：池外标的 | 通过（格式合法） | 拒绝整笔决策 | 预期错误码 `SYMBOL_NOT_IN_POOL` |
| 反例：现金不足 | 通过 | 拒绝整笔决策 | 预期错误码 `INSUFFICIENT_CASH` |
| 反例：非 100 股整数倍 | 通过 | 拒绝整笔决策 | 预期错误码 `LOT_SIZE_INVALID` |
| 反例：kill switch 开启 | 通过 | 拒绝整笔决策 | 预期错误码 `KILL_SWITCH_ACTIVE` |

反例不单独落文件，避免被误当作可执行样例；业务校验测试应在同一决策内混合合法/非法订单，并断言**没有任何订单被接受**。校验通过后的模拟撮合仍可能逐笔失败（停牌、涨跌停），这属于执行状态，不是“部分校验通过”。

`strategy-instance.example.yaml` 与 `agent-profile.example.yaml` 的资金/限额与成功样例保持一致；修改任一示例时必须同时重算订单金额。
