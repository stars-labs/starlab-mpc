# Starlab Key — ESP32 开源低成本 MPC 硬件分片固件

> 用一块 **¥25–¥75（\$3.5–\$10）** 的开源 ESP32-S3 / ESP32-C3 开发板，制作属于你自己的 **FROST 硬件冷分片**：  
> 私钥分片永不出片 · 硬件 TRNG 生成随机数 · AES-256-GCM 加密 NVS 存储 · 物理按键确认防木马静默盗签 · 连续输错 10 次 PIN 自动自毁擦除。

完整架构设计、BOM 采购清单、安全威胁模型与 `SLHW v1` 协议规范请见：[docs/ESP32_HARDWARE_SHARE.md](../../docs/ESP32_HARDWARE_SHARE.md)。

---

## 1. 零硬件先体验（宿主机模拟 ESP32）

还没买开发板？可以直接在本机跑一模一样的固件状态机（完整 FROST Ed25519 + secp256k1 Taproot 2 轮 DKG 与冷签）：

```bash
# 1) 跑全部跨端密码学互操作与安全自毁测试（ESP32 硬件分片 + starlab-core 软件节点 2-of-3 混签）
cargo test -p starlab-hw-protocol

# 2) 启动交互式虚拟 ESP32 串口模拟器（stdin/stdout 即模拟 /dev/ttyACM0）
cargo run -p starlab-hw-protocol --example esp32_sim_bridge
```

输入任意 `SLHW v1` 指令测试：
```json
{"id":1,"cmd":"device_info"}
{"id":2,"cmd":"set_pin","pin":"123456"}
{"id":3,"cmd":"dkg_init","wallet_id":"my-btc","curve":"secp256k1","participant_index":1,"threshold":2,"total":3}
{"id":4,"cmd":"dkg_round1","wallet_id":"my-btc"}
```

---

## 2. 推荐开发板（免焊接现货）

| 板型 | `--board` 参数 | 芯片 | 参考价 | 特点 |
| :--- | :--- | :--- | :---: | :--- |
| **LilyGO T-Display-S3** ⭐ | `t-display-s3` | ESP32-S3 (16MB/8MB PSRAM) | ¥55–¥75 | 自带 1.9" IPS 彩屏 + 左右 2 颗物理按键，零焊接即成品 |
| **Seeed XIAO ESP32-C3** | `xiao-c3` | ESP32-C3 (RISC-V) | ¥25–¥35 | 极小体积，外接 0.96" I2C OLED (`SDA=6, SCL=7`) + 2 颗按键 (`GPIO2/3`) |
| **CYD-S3 2.8" LCD** | `cyd-s3` | ESP32-S3 (16MB/8MB PSRAM) | ¥65–¥85 | 自带 2.8" 大屏 + MicroSD 卡槽，支持全气隙冷签 |
| **ESP32-S3-DevKitC-1** | `devkit-s3` | ESP32-S3 | ¥30–¥45 | 通用评估板，BOOT 键确认 + `GPIO4` 拒绝 |

---

## 3. 编译与一键刷机

```bash
# 1. 安装 Espressif Rust 工具链（仅首次需要）
cargo install espup espflash ldproxy
espup install
source ~/export-esp.sh

# 2. 插上开发板 USB-C，一键编译并刷机（以 LilyGO T-Display-S3 为例）
cd hardware/esp32-share
./flash.sh --board t-display-s3 --port /dev/ttyACM0
```

### 生产加固模式（不可逆熔断 eFuse，请先用普通模式验证通过！）
```bash
# 先生成 Secure Boot V2 签名密钥（妥善保管！）
espsecure.py generate_signing_key --version 2 secure_boot_signing_key.pem

# 开启 Flash Encryption (AES-256-XTS) + Secure Boot V2 + 禁用 JTAG
./flash.sh --board t-display-s3 --port /dev/ttyACM0 --release-secure
```
