# 演示彩排清单 — MPC 实时演示

> 英文原文 / English: [DEMO_REHEARSAL_CHECKLIST.md](./DEMO_REHEARSAL_CHECKLIST.md)

在面向投资人之前，请务必在**实际演示设备和网络上端到端完整跑通至少两次**。目标不是记住命令，
而是要在登台之前**提前**暴露那些枯燥的故障（房间 (room) 拼写错误、设备 ID (device-id) 重复、
Wi‑Fi 掉线、二进制冷启动）。请配合指南一起使用：[`INVESTOR_GUIDE.zh.md`](INVESTOR_GUIDE.zh.md)（下方章节号均指向它）。

> 经验法则（遵循本项目的演示原则）：**绝不在台上运行任何你没有在同一台机器上、
> 一字不差地彩排过的命令。**

---

## T‑1 天 — 准备工作

- [ ] **每台设备上都已构建二进制。** 在每台笔记本上运行 `cargo build --release -p mpc-wallet-cli`。
      确认 `--curve ed25519` 存在：`mpc-wallet-cli serve --help | grep -i curve`。
- [ ] **运行独立验证的机器上已安装 Node.js**（如果要展示链上交易，还需安装
      `@solana/web3.js`：在仓库根目录运行 `bun install`）。
- [ ] **确定要讲的曲线方案：** ed25519（Solana）——它是可以独立验证的那一个。
- [ ] **预先为 Solana 地址充值**（仅当要演示链上 `finalize` 环节时）。公共 devnet 水龙头有速率限制，
      因此切勿依赖现场 airdrop。请通过 <https://faucet.solana.com> 预先充值，或从一个已有余额的
      devnet 钱包转账，并记录该地址。（地址是按每次 DKG 运行生成的，因此要么预先创建一个持久化钱包
      并复用其 keystore，要么计划将链上环节作为*不依赖资金*的 `verify` 证明来展示——见指南 §3.4。）
- [ ] **写下一份固定方案：** 设备 ID (device-id)（`alice`/`bob`/`carol`）、房间 (room) ID、门限
      (2‑of‑3)、演示密码。每台机器使用唯一的设备 ID (device-id)——**重复的 ID 会悄无声息地破坏网络。**
- [ ] **给所有设备充满电 / 准备好电源适配器。** 演示往往时间很长。

## T‑10 分钟 — 在实际网络上，每台设备都执行

- [ ] **每台笔记本上预检 (preflight) 通过：**
      `SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh` → **8 passed, 0 failed**，
      其中包括第 4 步（通过实时服务器跑一次真实仪式）。如果第 4 步是红色的，说明实时通路已宕机
      → 立即切换到 LAN 降级方案 (fallback)（指南 §6 第 1 级）。
- [ ] **统一确认：** 每个人都已粘贴*相同*的房间 (room) ID 和信令服务器 URL，准备就绪。
- [ ] **投影 / 屏幕镜像正常工作**；终端字体足够大、可读。
- [ ] **降级方案 (fallback) 就绪：** 明确 Wi‑Fi 掉线时由哪台笔记本运行 LAN 信令服务器，
      并且核弹级的 `scripts/demo/ceremony.sh … --sign` 单行命令一粘即用（指南 §6 第 3 级）。

## 正式跑通 — 跑两次，计时

1. [ ] 三个终端启动 `serve --curve ed25519`（指南 §3.1）。三者都显示 `connection: true`。
2. [ ] alice 执行 `create_wallet` → 大声读出 `session_announced` 的 ID。
3. [ ] bob + carol 执行 `join_session`。**三者都打印出相同的 `group_public_key`。** ✅
       *（瞄一眼：三个 key 是否一致？这就是现场"它是真的"的高光时刻。）*
4. [ ] 展示三个独立的 keystore 文件确实存在（`ls ~/.frost_*`）。
5. [ ] 投资人指定一条消息 → alice 执行 `sign` → bob 执行 `approve_signing` → `signature_complete`。
6. [ ] 在一台干净的机器上**独立验证**（指南 §3.3）→ `VERIFIED: true`。
7. [ ] （如使用）链上环节（指南 §3.4）：`verify` → `verifySignatures: true`，
       以及/或 `finalize` → 打开浏览器链接并显示已确认的交易。
8. [ ] **门限戏剧性演示**（指南 §3.5）：alice 尝试独自签名 → 超时 → 加上 bob 重试 → 完成。
9. [ ] **恢复环节**（指南 §5，可选）：在持有钱包的节点上运行
       `mpc-wallet-cli reshare --wallet-id <W> --room "$ROOM"`，让保留下来的签名方
       `session join` 它 → 相同的 `group_public_key`、钱包继续可签、被移除设备的分片作废。
       即"丢失/轮换一台设备，地址不变"的故事。*（没有现场环境？重新分享引擎已在
       `cargo test -p mpc-wallet-cli` 中验证。）*

## 故障演练（要彩排恢复流程，而不只是顺利路径）

- [ ] **中途切断 Wi‑Fi** → 将整组切换到 LAN 服务器（第 1 级）。计时。
- [ ] **在某个节点上输入错误的房间 (room)** → 看到它始终无法加入；冷静地现场修复。
- [ ] **"核弹级"冷启动开场** → 从零开始，运行 `scripts/demo/ceremony.sh … --sign` 单行命令
      并验证输出。这是你"无论如何，这里都有可用的密码学 —— 真实的独立进程"的底牌。
- [ ] **水龙头失效**（如演示链上）→ 降级到不依赖资金的 `verify` 证明。

## 放行确认（全部为真之前不要登台）

- [ ] 在真实设备 + 网络上，从头到尾完整跑通顺利路径**两次**。
- [ ] 两次独立验证都返回 `true`。
- [ ] 至少彩排过一次故障演练并干净地恢复。
- [ ] 每位操作员都清楚自己的确切台词以及降级方案 (fallback) 阶梯。
- [ ] 每个工位都放有一份指南 **§10 快速参考卡**的纸质打印件。

---

### 时间目标
一次干净的跑通（启动节点 → DKG → 签名 → 独立验证）实际工作量约为 **60–90 秒**。
要给讲解留出时间预算；密码学本身很快（DKG + 签名在个位数秒内完成）。如果耗时达到几分钟，
说明网络出了问题——果断降级，而不要在全场面前僵住等待。
