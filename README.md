# CandyMachine (糖果机) 🍬

CandyMachine 是一款基于 Solana 区块链的去中心化公平抽奖协议。它模仿了自动化糖果机的逻辑：用户投入资产（SOL/USDT），机器通过公平的随机算法掉落不同价值的“糖果”奖金。

## 🌟 核心特性

- **多币种支付**：支持原生 SOL（基于 Pyth 实时汇率换算）或 USDT 支付。
- **透明随机性**：集成 MagicBlock Ephemeral VRF，确保每个“糖果”的掉落过程链上可验证、防攻击。
- **阶梯奖池系统**：采用 4 层概率分布模型，单次投入（10 USD）最高可掉落价值 99.9 USD 的奖金。
- **自动化兑付**：支持直接领取 SOL，或通过 Jupiter/Raydium 自动兑换为指定的 Token 奖励。
- **安全机制**：内置超时退款保障与管理员紧急管理权限。

## 🏗 架构设计

### 核心模块

- **Admin (管理员)**: 负责糖果机初始化、参数调整及奖池配置。
- **User (用户)**: 投入资产（RequestMint）、领取奖金（Claim）及超时退款（Refund）。
- **Oracle (预言机)**: 处理 VRF 随机数回调逻辑，确定“糖果”成色。
- **Utils (工具类)**: 集成 Pyth 汇率、Jupiter/Raydium DEX 交互等。

### 状态管理 (PDA)

- **Global Config**: `[b"global_config"]` - 机器全局状态及活跃奖池列表。
- **Mint Request**: `[b"mint_request", user, slot]` - 记录用户的单次抽奖状态与结果。
- **Vault**: `[b"vault"]` - 存储投入资产与待发放奖金的协议金库。
- **Prize Pool**: `[b"prize_pool", index]` - 具体的糖果仓（DEX 池子）配置。

## 🎲 掉落概率 (10 USD/次)

| 奖励等级 | 概率 | 奖金范围 (USDC) |
| :--- | :--- | :--- |
| **Tier 1 (紫色糖果)** | 15% | 5.0 - 7.0 |
| **Tier 2 (蓝色糖果)** | 50% | 7.0 - 14.0 |
| **Tier 3 (绿色糖果)** | 30% | 14.0 - 49.9 |
| **Tier 4 (金色糖果)** | 5% | 50.0 - 99.9 |

**预期 ROI**: ~194.8% (旨在通过高回报率吸引流量)。

## 🚀 交互流程

1. **投币 (RequestMint)**: 用户支付 10 USD 价值的资产，触发 VRF 请求。
2. **开奖 (VRF Callback)**: 随机数生成并回传，合约锁定中奖等级及金额。
3. **出货 (Claim)**: 用户选择心仪的币种（SOL 或 Token），机器执行发放。
4. **退币 (Refund)**: 若机器发生异常（VRF 超时），用户可撤回投入的资产。

## 🛠 开发与部署

### 环境
- Anchor Framework 0.30.1
- Solana CLI 1.18+

### 常用命令
```bash
anchor build
anchor deploy
```

## 📜 许可证

本项目遵循 MIT 许可证。