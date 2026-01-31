# rust-silk

SILK v3 编解码工具与 Rust 封装。内置官方 SILK C 源码（BSD 3-Clause），提供易用的 CLI，
覆盖常见的微信语音（含腾讯前缀）与 PCM/WAV 互转场景。

## 特性

- **编解码**：SILK ↔ PCM s16le/WAV（单声道 16-bit）
- **腾讯前缀兼容**：支持 `0x02 + #!SILK_V3`
- **容错解码**：坏包跳过或静音填充
- **指标输出**：解码 metrics + 与参考音频的能量/信噪比对比
- **标准输入输出**：`-i -` / `-o -`

## 安装

```bash
cargo install rust-silk
```

或从源码构建：

```bash
cargo build --release
```

## 用法

```text
rust-silk <encode|decode> [options]
```

### 编码（PCM/WAV → SILK）

```bash
rust-silk encode -i input.wav -o output.silk --sample-rate 24000
```

写腾讯前缀（微信语音常见）：

```bash
rust-silk encode -i input.pcm -o output.silk --tencent
```

### 解码（SILK → PCM/WAV）

```bash
rust-silk decode -i input.silk -o output.pcm --sample-rate 24000
```

输出 WAV：

```bash
rust-silk decode -i input.silk -o output.wav
```

容错模式（坏包跳过或静音填充）：

```bash
rust-silk decode -i input.silk -o output.pcm --tolerant skip
rust-silk decode -i input.silk -o output.pcm --tolerant silence
```

### 指标/对比

```bash
rust-silk decode -i input.silk -o output.wav --metrics --reference ref.wav
```

## 采样率说明

支持：8000 / 12000 / 16000 / 24000 / 48000 Hz。
若输入 WAV 采样率不匹配，将提示调整。

## License

本项目整体遵循 BSD 3-Clause；其中包含的 SILK C 源码版权归 Skype Limited 所有。
详见 `LICENSE` 与源码头部声明。
