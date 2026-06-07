# 投资人演示指南 — 多设备 MPC 钱包

> 英文原文 / English: [INVESTOR_GUIDE.md](./INVESTOR_GUIDE.md)
> 配合彩排清单使用：[`DEMO_REHEARSAL_CHECKLIST.zh.md`](./DEMO_REHEARSAL_CHECKLIST.zh.md)。

**目标：** 在投资人面前，证明这是**运行在真实、彼此独立的多台机器上的真正门限密码学** —— 多个人共同创建一个钱包（任何单个人都不掌握私钥），共同签名，并由一个**不是我们自己写的**工具来验证，而且**全程现场不出任何岔子。**

演示有**两种跑法**，按受众挑选，或两种都做：

- **A 线 —— 原生 CLI，可独立验证**（§3）。这是头条卖点。每条命令都是原始且可见的；最终的签名由投资人**自己的**密码学库（Node.js / Python / OpenSSL）来验证，而且密钥是一个**真实的 Solana 地址**。这是无法造假的版本 —— 面对持怀疑态度的人时优先用它。
- **B 线 —— TUI 多设备**（§4）。视觉上更精致：每个人在自己的笔记本上操作一个终端界面，包含物理隔离（air-gap）和多链场景。密码学是一样的，更有表演性，但可独立验证性弱一些。

整体策略分三层：**彩排 + 起飞前检查**（先在私下证明它能跑通）、**现场演示**，以及一套**降级方案（fallback）**，其最底层一级绝不会失败。

---

## 0. 黄金法则（请务必阅读，否则一定会踩坑）

- **每个节点都需要一个唯一的 `--device-id`。** 两个人选了相同的 id（或都让它默认成相同的主机名），会在信令服务器上冲突，网状连接（mesh）会无声地断掉。提前分配好名字：`alice`、`bob`、`carol`…… 把它们发下去。
- **演示开始前 10 分钟运行 `preflight.sh`**（§2）。绿灯 = 密码学 + WebRTC + 网络路径都健康。红灯 = 你在私下、而不是台上发现了问题。
- **一个共享房间（room），只生成一次。** 托管服务器要求一个强 `--room`（≥16 个字符）。只生成一次，把完全相同的值发给每位参与者，并在每台设备上传入。不同的房间彼此看不见；不带房间的裸 URL 会被拒绝。（这同时也是租户隔离边界 —— 请让每一批人的房间保持私有。）本地服务器降级方案（§6 第 1 级）**不需要**房间。
- **提前确定信令服务器**，并把它配到每台设备上：
  - 在线：`--signal-server wss://panda.qzz.io --room "$ROOM"`（需要联网）。
  - 本地备份：一台笔记本运行服务器，其他人都用 `--signal-server
    ws://<那台笔记本的局域网-ip>:9000`（需要同一 Wi-Fi，不需要联网）。在会议*之前*就把它配好，这样你能在 10 秒内切换过去。
- **用 ed25519 演示。** 一个 FROST-ed25519 门限签名就是一个*完全标准*的 RFC-8032 Ed25519 签名，因此投资人可以用任何现成的库来验证它（§3.3）。（在 secp256k1 上我们的签名是 RFC-9591，没有现成工具能校验它 —— 所以我们刻意用 ed25519 来演示。）
- **永远不要在幻灯片上输入真实密码。** 用一个用完即弃的密码，比如 `demo`。
- **始终备好一台笔记本用于降级方案**（§6）。

---

## 1. 准备与前置条件（每台设备，需事先完成）

```bash
# once per device
git clone <repo> && cd mpc-wallet
nix develop                      # or have the toolchain installed

# Track A (raw CLI): build the CLI
cargo build --release -p mpc-wallet-cli      # binary: ./target/release/mpc-wallet-cli
# Track B (TUI): build the TUI
cargo build --release -p tui-node
```

- 在线路径需要**联网**（`wss://panda.qzz.io`）。没有网络？见 §6 第 1 级。
- A 线需要在投资人的机器上装一个**验证工具**：**Node.js**（最简单），或带 `cryptography`/`PyNaCl` 的 Python，或 `openssl`。全部见 §3.3。
- 如果你要展示上链环节（§3.4），必须已安装 `@solana/web3.js`（在仓库根目录运行 `bun install`）。

**共享房间** —— 由一个人生成，并把确切的值发给所有人：

```bash
ROOM=$(uuidgen | tr -d -)      # e.g. 7f3a9c2e4b1d4e8a9c2f001122334455
echo "$ROOM"                   # send this exact string to bob and carol
```

> 托管服务器（`panda.qzz.io`）**要求一个强 `--room`**（≥16 个字符；见 #31）。
> 同一个钱包的所有参与者都必须传入**相同的**房间。裸的 `wss://panda.qzz.io`
>（不带房间）会被拒绝。

2-of-3（门限：任意两台即可签名）演示的角色：**alice、bob、carol** —— 任意两人即可签名，任何一人单独无法签名。是个好故事。

---

## 2. 起飞前检查（T-10 分钟）

在任意一台机器上：

```bash
scripts/demo/preflight.sh
# or against the server you'll actually use:
SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh
```

它会**端到端**跑完整的 DKG（2-of-2 / 2-of-3 / 3-of-5）和门限签名，每个只需几秒，然后检查信令服务器是否可达。全部 ✅ → 可以开始。任何 ❌ → 修好它，或降级（§6）。这是最重要的一步 —— 它就是对"出事怎么办"的回答：你先在私下确保根本不会出事。

> 为什么它值得信任：`preflight` 使用 `mpc-wallet-cli simulate`，它在一个进程里拉起一个真实的
> N 节点 FROST 仪式（真实的密码学、走环回（loopback）的真实 WebRTC、内嵌的服务器）。
> 它与真实客户端走的是同一条代码路径。

---

## 3. A 线 —— 原生 CLI，可独立验证（头条卖点）

最有力的演示：三个彼此独立的进程（理想情况下是三台笔记本），每个都有自己的密钥库（keystore）文件，运行一次真实的 DKG + 门限签名，然后由一个**投资人信任的**、而非我们的工具来确认签名。

> **它回答了什么** —— 怀疑者的质疑：
>
> | 投资人的疑虑 | 演示如何回应 |
> |---|---|
> | "这是一个程序在假扮多方。" | **三个彼此独立的进程**，每个都有**自己的密钥库文件**，各自独立打印结果。 |
> | "你的工具说有效 —— 也许是你的工具在撒谎。" | 由 **Node.js / Python / OpenSSL 内置密码学**验证 —— 不是我们写的代码 —— 而且密钥是一个**真实的 Solana 地址**。 |
> | "也许有一台机器偷偷掌握了整把密钥。" | **没有任何一台机器能单独签名。** 在一个 2-of-3 钱包中，单台设备独自签名只会**超时**（§3.5）。 |
> | "这是预录的 / 提前准备好的。" | 由投资人**当场指定消息**。签名随之改变；它仍然能通过验证。 |
> | "密钥是一次性生成后硬编码进去的。" | **现场通过 DKG 创建一个全新的钱包**；密钥每次运行都不同，并依赖于三台机器各自的随机数。 |

> **协议传输层。** 每个节点运行 `mpc-wallet-cli serve`，它说的是
> **以换行符分隔的 JSON**：你在 stdin 上输入一个命令对象，它在 stdout 上打印事件对象。
> 投资人字面上能看到线路协议 —— 没有任何隐藏。

### 3.1 启动三个节点

每个人运行**一条**命令（替换成各自的名字）。让这些终端保持打开且可见。

```bash
# alice
./target/release/mpc-wallet-cli serve --curve ed25519 \
  --device-id alice --keystore ~/.frost_alice \
  --signal-server wss://panda.qzz.io --room "$ROOM"

# bob
./target/release/mpc-wallet-cli serve --curve ed25519 \
  --device-id bob --keystore ~/.frost_bob \
  --signal-server wss://panda.qzz.io --room "$ROOM"

# carol
./target/release/mpc-wallet-cli serve --curve ed25519 \
  --device-id carol --keystore ~/.frost_carol \
  --signal-server wss://panda.qzz.io --room "$ROOM"
```

每个节点都会打印：
```json
{"event":"ready","protocol":1,"device_id":"alice","curve":"ed25519"}
{"event":"connection","connected":true}
```

### 3.2 创建钱包，然后签名（现场仪式）

下面的所有内容都输入到某个节点的终端里（它的 stdin）。输入 JSON 后按回车。

**alice 创建一个 2-of-3 钱包（分布式密钥生成）：**
```json
{"cmd":"create_wallet","threshold":2,"total":3,"password":"demo"}
```
alice 会打印会话 id —— **念出来**：
```json
{"event":"session_announced","session_id":"dkg_8f1c…"}
```

**bob 和 carol 加入那个会话**（使用 alice 刚刚宣布的 id）：
```json
{"cmd":"join_session","session_id":"dkg_8f1c…","password":"demo"}
```
几秒钟后，**三个**终端各自独立地打印出相同的结果：
```json
{"event":"dkg_complete","wallet_id":"…","address":"<Solana base58 address>","group_public_key":"<64 hex chars>"}
```

> 🎤 **话术：** "三台彼此独立的机器刚刚共同生成了一个共享钱包。没有谁掌握完整的私钥 ——
> 每人只持有一个*密钥分片（share）*。三台机器各自独立地打印出**相同的**公钥和**相同的**
> Solana 地址。"
>
> 展示三个密钥库文件确实存在且各不相同：
> ```bash
> ls -la ~/.frost_alice ~/.frost_bob ~/.frost_carol   # three separate shares on disk
> ```

**对投资人选定的消息进行签名。** 向投资人要一句话；把它放进 **alice 的**终端：
```json
{"cmd":"sign","wallet_id":"<wallet_id from dkg_complete>","message":"we closed the round","encoding":"utf8","password":"demo"}
```
**bob** 会看到一个批准请求并同意（使用 bob 打印出的 `sign_…` id）：
```json
{"event":"signing_request","session_id":"sign_3a2e…","wallet":"…"}
```
```json
{"cmd":"approve_signing","session_id":"sign_3a2e…","password":"demo"}
```
alice 打印出完成的签名：
```json
{"event":"signature_complete","signature":"0x<128 hex chars>","message_hash":"…"}
```

> 🎤 **话术：** "三台设备中的两台刚刚共同签署了投资人指定的那条确切消息。让我们来证明它是真的 ——
> 用一个我们谁都没写过的工具。"

### 3.3 决定性时刻：独立验证

把这次运行中的**三个值**交给投资人：

- **GK** —— 来自 `dkg_complete` 的 `group_public_key`（64 个十六进制字符）
- **SIG** —— 来自 `signature_complete` 的 `signature`，**去掉开头的 `0x`**（128 个十六进制字符）
- **MSG** —— 你所签名的那条确切消息字符串（例如 `we closed the round`）

投资人在**他们自己的机器上**运行下列**任意一条**：

**Node.js（内置 `crypto`，无需安装）**
```bash
node -e '
const crypto=require("crypto");
const GK="PASTE_GK", SIG="PASTE_SIG_NO_0x", MSG="we closed the round";
const der=Buffer.concat([Buffer.from("302a300506032b6570032100","hex"),Buffer.from(GK,"hex")]);
const pub=crypto.createPublicKey({key:der,format:"der",type:"spki"});
console.log("VERIFIED:", crypto.verify(null, Buffer.from(MSG), pub, Buffer.from(SIG,"hex")));
'
# → VERIFIED: true
```

**Python（`cryptography`）**
```bash
python3 -c '
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
GK="PASTE_GK"; SIG="PASTE_SIG_NO_0x"; MSG=b"we closed the round"
Ed25519PublicKey.from_public_bytes(bytes.fromhex(GK)).verify(bytes.fromhex(SIG), MSG)
print("VERIFIED: True")   # raises + prints nothing if invalid
'
```

**Python（`PyNaCl`）**
```bash
python3 -c '
from nacl.signing import VerifyKey
GK="PASTE_GK"; SIG="PASTE_SIG_NO_0x"; MSG=b"we closed the round"
VerifyKey(bytes.fromhex(GK)).verify(MSG, bytes.fromhex(SIG)); print("VERIFIED: True")
'
```

**OpenSSL**
```bash
# portable hex→binary helper (works without xxd):
hex2bin(){ python3 -c "import sys,binascii;open(sys.argv[2],'wb').write(binascii.unhexlify(sys.argv[1]))" "$1" "$2"; }

hex2bin "302a300506032b6570032100PASTE_GK" pub.der     # 12-byte SPKI prefix + key
openssl pkey -pubin -inform DER -in pub.der -out pub.pem
printf '%s' "we closed the round" > msg.bin
hex2bin "PASTE_SIG_NO_0x" sig.bin
openssl pkeyutl -verify -pubin -inkey pub.pem -rawin -in msg.bin -sigfile sig.bin
# → Signature Verified Successfully
```

**附赠：它是一个真实的 Solana 地址。** 来自 `dkg_complete` 的 `address` 就是那同一把 32 字节密钥的 base58 编码 —— 一个有效的 Solana 账户。把它粘贴到
<https://explorer.solana.com>，展示它是一个真实、格式良好、由这个钱包控制的账户。

> 🎤 **收尾语：** "是你自己的密码学库 —— 不是我们的 —— 刚刚确认了：一个对*你*所选消息的签名，
> 在一把本身就是真实 Solana 地址的密钥下是有效的，而它是由三台各自只持有一个碎片的独立机器中的
> 两台共同生成的。这就是门限 MPC，现场演示。"

### 3.4 进阶：一笔真实的 Solana 区块链交易

最有力的版本：MPC 钱包签署一笔真实的 Solana 转账，并让它上链，可在公开的区块浏览器中看到。分工（这正是它可信的原因）：由**标准的 `@solana/web3.js` 库**来构建并提交交易；我们的**原生 `mpc-wallet-cli` 只负责签名**。辅助脚本：`scripts/demo/solana_onchain.mjs`。

> **始终从 `group_public_key` 派生地址**（`solana_onchain.mjs address
> <groupKeyHex>`），而不是从 `dkg_complete` 事件的 `address` 字段派生 —— 该字段目前对 ed25519
> 而言不可靠（单独跟踪中）。

> **演示前（在你上台之前就做好 —— 现场水龙头有速率限制）。** 提前给地址注资：
```bash
node scripts/demo/solana_onchain.mjs address <groupKeyHex>     # -> the Solana address
# then fund it via the web faucet (https://faucet.solana.com, has a captcha)
# or transfer ~0.01 SOL from any funded devnet wallet. Confirm it's funded:
node scripts/demo/solana_onchain.mjs airdrop <groupKeyHex> 1   # works only if not rate-limited
```

**台上 —— 两种呈现方式（按你是否提前注资来挑选）：**

(i) **与注资无关的证明**（不会被速率限制 —— 推荐的安全默认选项）：
```bash
node scripts/demo/solana_onchain.mjs prepare <groupKeyHex> self 1000   # prints MESSAGE hex
# MPC-sign that message (2-of-3): in alice's serve terminal
#   {"cmd":"sign","wallet_id":"…","message":"<MESSAGE hex>","encoding":"hex","password":"demo"}
#   bob: {"cmd":"approve_signing","session_id":"sign_…","password":"demo"}
node scripts/demo/solana_onchain.mjs verify <signatureHex>             # -> web3.js tx.verifySignatures(): true
```
> 🎤 "标准的 Solana 库刚刚确认了我们的门限签名对一笔真实的 Solana 交易是有效的 ——
> 无需信任我们的代码。"

(ii) **完整上链**（如果地址已提前注资）：同样的 `prepare` + MPC 签名，然后
```bash
node scripts/demo/solana_onchain.mjs finalize <signatureHex>          # submits; prints the explorer URL
```
在投影仪上打开打印出来的 `https://explorer.solana.com/tx/…?cluster=devnet` 链接。
> 🎤 "那笔交易刚刚在一条公开区块链上结算完成 —— 由三台从未组装出完整私钥的机器中的两台授权。"

> `prepare → sign → finalize` 必须在约 60 秒内完成（blockhash 的有效期），所以要让 MPC
> 节点提前处于运行状态。

### 3.5 "单台设备无法独自签名"（最直击人心的证明）

1. 在 **alice 的**终端，发起一次签名**但让任何人都不批准**：
   ```json
   {"cmd":"sign","wallet_id":"…","message":"alice alone","encoding":"utf8","password":"demo"}
   ```
2. 等待。门限为 2 而只有 alice 参与，仪式**无法完成** —— 它会超时，没有签名。
3. 现在让 bob 批准并重试一次 → 它完成了。

> 🎤 "单台机器，靠它自己，是无能为力的。门限是由数学强制执行的，而不是由策略。"

---

## 4. B 线 —— TUI 多设备（精致的视觉版）

每位参与者在自己的笔记本上启动终端界面：

```bash
cargo run --release --bin mpc-wallet-tui -p tui-node -- \
  --device-id alice \
  --signal-server wss://panda.qzz.io \
  --room "$ROOM"
```

### 场景 1 —— 在线 DKG（创建一个共享钱包）
1. **alice**：Create Wallet → 2-of-3 → 设个名字 → 设密码 → 它会宣布一个会话。
2. **bob**、**carol**：出现一条 "session available" 通知 → Join → 输入各自的密码。
3. 当 3 个人都加入后，DKG 开始运行（几秒钟）。
4. **关键看点：** 三块屏幕显示**相同的钱包地址**，然而**没有任何一台设备掌握过私钥** ——
   每人只持有一个分片。这就是 MPC。

### 场景 2 —— 门限签名（共同签名）
1. **alice**：打开钱包 → 对一条消息/交易签名。
2. **bob**：收到一个签名请求 → Approve（输入密码）。
3. 由 alice + bob 的分片生成一个有效签名。
4. **关键看点：** carol 没有参与，也不需要她（2-of-3）。可以选择性地展示 **alice 独自无法**
   生成签名 —— 门限是由数学、而非策略强制执行的。

### 场景 3 —— 离线 / 物理隔离（air-gap，SD 卡）
1. 在 TUI 中切换到**离线模式**（物理隔离的 DKG/签名）。
2. 每位参与者把自己那一轮的数据包导出到一张 SD 卡 / U 盘上。
3. 在机器之间物理传递这张卡；逐轮导入。
4. **关键看点：** 密钥的生成与使用，全程机器**从未连接任何网络** —— 冷存储 / 高安全的故事。

### 场景 4 —— 多个钱包 + 多链地址
1. 打开 alice 的钱包详情 → 展示由同一套分片派生出来的 **ETH / BTC / Solana** 地址。
2. 创建**第二个**钱包（不同门限，例如 3-of-5），表明这不是一锤子买卖。
3. **关键看点：** 一套 MPC 密钥 → 覆盖每条主流链的地址；钱包数量不限。

---

## 5. 恢复与轮换："丢一台设备不等于丢钱包"

这是每个投资人面对多设备钱包都会问的问题。**密钥分片刷新 / 重新分享（reshare）**的密码学引擎已经交付，并可用一条命令演练 —— 它无需任何网络配置即可证明恢复的故事：

```bash
# Rotate all shares (proactive security) — same wallet, fresh shares:
mpc-wallet-cli reshare-simulate --nodes 3 --threshold 2 --curve ed25519

# Remove a lost/stolen device (2-of-3 → keep only devices 1 & 2):
mpc-wallet-cli reshare-simulate --nodes 3 --threshold 2 --curve ed25519 --keep 1,2
```

两者都打印相同的结构：
```json
{ "kept": [1,2], "group_public_key": "06833fdf…badb6ac8",
  "key_preserved": true, "refreshed_quorum_signs": true, "old_share_rejected": true, "ok": true }
```

要指给投资人看的点：
- **`group_public_key` 前后完全相同** → **地址永不改变**；资金不动，无需重新注资。
- **`refreshed_quorum_signs: true`** → 钱包用新的分片继续正常工作。
- **`old_share_rejected: true`** → 刷新之前的每一个分片现在都**作废了** —— 被盗设备的碎片
  再也无法组合起来签名。

> 🎤 "丢了一台笔记本？刷新到幸存的设备上 —— 地址不变，钱包继续工作，丢失设备的分片现在一文不值。
> 我们还能按计划定期轮换，这样一个用几个月时间收集碎片的攻击者永远拼不出一把密钥。
> 单密钥的托管钱包做不到这其中任何一点。"

> **范围：** `reshare-simulate` 在**一个进程内**运行真实的刷新（就像核选项级别的降级方案）——
> 它证明的是密码学本身。**联网的**多设备 reshare 仪式（走 WebRTC 网状连接，类似 DKG）也已交付 ——
> 用 `mpc-wallet-cli reshare --wallet-id <W>` 发起，让保留下来的签名方执行 `session join`
>（或 `serve --auto-approve`）。完整的威胁模型 + 话术见
> `docs/RECOVERY_AND_RESHARING.md`。

---

## 6. 降级方案（fallback）（当现场出现抖动时）

一级一级地往下降。每一级都比上一级更可靠、更不依赖网络；最底层一级是万无一失的。把第 2~3 级彩排到位，让切换成为肌肉记忆。

| 级别 | 触发条件 | 操作 |
|---|---|---|
| **0. 在线** | 正常 | 通过 `wss://panda.qzz.io` + 一个共享 `--room` 进行多设备演示。 |
| **1. 本地 / 局域网服务器** | 网络不稳 / panda 不可达 | 一台笔记本：`MPC_SIGNAL_BIND=0.0.0.0:9000 cargo run --release -p webrtc-signal-server`。所有人用 `--signal-server ws://<笔记本的局域网-ip>:9000` 重启（本地服务器**不需要**房间）。同样的演示，无需互联网。 |
| **2. 单台笔记本，视觉版**（TUI） | 某位参与者的设备出问题 | `scripts/demo/demo-local.sh` → 在一台机器上、用 tmux 网格跑本地服务器 + 3 个 TUI 节点。看起来仍然像多方。 |
| **3. 核选项（绝不失败）** | 一切都着火了 | `./target/release/mpc-wallet-cli simulate --nodes 3 --threshold 2 --curve ed25519 --sign "we closed the round"` → 约 3 秒内完成完整的 DKG + 签名 + 一个可验证的签名，自包含。它打印群公钥 + 一个已验证的签名；然后用 §3.3 来验证。"这就是密码学，此刻正在工作。" |

---

## 7. 故障排查（速查）

| 症状 | 原因 / 修复 |
|---|---|
| `WebSocket … 400` / 连接被拒 | 缺少或过弱的 `--room`（托管服务器要求 ≥16 个字符）。用 `uuidgen \| tr -d -` 生成；所有人用**同一个**值。 |
| 某个节点一直不加入 / 永远卡在 "Waiting for participants" | 重复的 `--device-id`，或者并非所有人都在**同一个房间 + 同一台服务器**上。给每个节点一个唯一 id；确认 `--signal-server` 和 `--room` 完全一致；检查网络 / 改用第 1 级。 |
| bob/carol 一直看不到会话 | 他们是在 alice 宣布之后才连上的 —— 他们直接用 alice 打印出的 id 发送 `join_session` 即可（迟到加入是可以的）。 |
| 验证器说 **false** | `MSG` 字节不对（必须是**确切**被签名的那条消息）、`SIG` 没去掉 `0x`，或者 GK/SIG 抄写有误。重新复制一遍。 |
| 签名能验证通过，但地址看起来很奇怪 | `address` 是 base58（Solana）；`group_public_key` 是十六进制 —— 同一把密钥，两种编码。 |
| TUI 中地址错误/奇怪 | 构建过期；重新构建 release（地址由 ETH/BTC/SOL 黄金测试钉死）。 |
| 批准后签名卡住 | 冷启动竞态（已修复）—— 重新构建到最新；若能复现，降到第 2/3 级。 |

---

## 8. 30 秒"出事了"决策树

1. 之前的**起飞前检查**通过了吗？如果没有 —— 你本就不该开始；直接到第 3 级。
2. 在线运行卡住超过约 30 秒？→ **第 1 级**（本地/局域网服务器），让所有人重新连接。
3. 仍然卡住，或者某台设备就是问题所在？→ **第 2 级**（单笔记本 TUI）或 **第 3 级**。
4. 还是有问题？→ **第 3 级**（核选项 simulate）。它一定能行。一边重置，一边讲解密码学。

现场调试绝不要超过约 30 秒。降一级，让故事继续推进，事后再修。

---

## 9. 你在主张什么 —— 以及底层原理（Q&A）

**主张：**
- **没有单点失陷：** 私钥从不存在于任何单一地点 —— 不在某台设备上，不在某台服务器上，
  甚至在签名过程中也一刻都不会存在。
- **门限由密码学强制执行：** k-of-n 是 FROST 数学；你可以丢失多达 n−k 台设备仍能签名，
  而攻击者需要 k 个分片。
- **支持离线：** 通过可移动介质完成完整的物理隔离仪式。
- **多链：** 一套分片 → ETH/BTC/Solana（及更多）地址。

**底层原理（面向技术型投资人）：**
- **DKG（密钥生成）：** FROST 分布式密钥生成，Pedersen 变体 —— **无需可信发牌方**。
  每台设备贡献随机数；私钥从不在任何地方被组装。每台设备最终得到一个*分片*；群公钥是公开的。
- **签名：** FROST 门限 Schnorr。`n` 台设备中的 `t` 台各自产生一个部分签名；
  这些部分签名聚合成**一个普通的签名**，可用标准验证器在群公钥下通过验证。
  没有任何设备会看到另一台的分片。
- **曲线：** ed25519（RFC 8032）—— 群公钥是一个普通的 Ed25519 公钥（一个 Solana 地址），
  签名是一个普通的 Ed25519 签名，因此才有 §3.3 中的独立验证。同一套软件也能运行 secp256k1
 （以太坊/比特币系）；我们之所以特意用 ed25519 演示，是因为它能用现成工具验证。
- **传输：** 用一个信令服务器做发现 + 用 WebRTC 承载分片所走的加密点对点网状连接。
  信令服务器从不接触密钥材料。
- **恢复 / 托管：** 因为没有发牌方，每台设备上加密的密钥库就是备份单元。
  （见 `docs/MULTI_CURVE_DERIVATION.md`。）

---

## 10. 速查卡（打印出来）

```
SHARED ROOM:  ROOM=$(uuidgen | tr -d -)        # same value on every device
SERVER:       wss://panda.qzz.io               # or LAN: ws://<laptop-ip>:9000 (no room)
ROLES:        alice / bob / carol   (2-of-3)   # UNIQUE --device-id each
PASSWORD:     demo                             # throwaway, never a real one

PRE-FLIGHT:   SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh     # all ✅ → go

— Track A (raw CLI) —
START (each):  mpc-wallet-cli serve --curve ed25519 --device-id <name> \
                 --keystore ~/.frost_<name> --signal-server wss://panda.qzz.io --room "$ROOM"
alice:         {"cmd":"create_wallet","threshold":2,"total":3,"password":"demo"}
bob,carol:     {"cmd":"join_session","session_id":"<dkg_…>","password":"demo"}
alice:         {"cmd":"sign","wallet_id":"<…>","message":"<investor's words>","encoding":"utf8","password":"demo"}
bob:           {"cmd":"approve_signing","session_id":"<sign_…>","password":"demo"}
VERIFY:        node -e '…'   # GK + SIG(no 0x) + MSG → VERIFIED: true

FALLBACK:      0 live → 1 LAN server → 2 one-laptop TUI → 3 nuclear simulate
NUCLEAR:       mpc-wallet-cli simulate --nodes 3 --threshold 2 --curve ed25519 --sign "we closed the round"
```
