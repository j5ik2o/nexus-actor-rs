# RP2040/RP2350 実機テスト環境メモ

## 必要ツール
- Arm GNU Toolchain (arm-none-eabi) 公式配布版
  - 例: `arm-gnu-toolchain-14.3.rel1-darwin-arm64-arm-none-eabi`
- Raspberry Pi Pico SDK（`pico-sdk`）
- `probe-rs` ツール群（`probe-rs run` 等）
- DebugProbe/Picoprobe 用ファーム（`debugprobe` リポジトリでビルド）

## 環境変数の設定例
```bash
export PICO_SDK_PATH=~/Sources/pico-sdk
export PICO_TOOLCHAIN_PATH=~/toolchains/arm-gnu-toolchain-14.3.rel1-darwin-arm64-arm-none-eabi
export PICO_GCC_TRIPLE=arm-none-eabi
export PATH="$PICO_TOOLCHAIN_PATH/bin:$PATH"
```

## DebugProbe のビルド手順（参考）
```bash
cd ~/Sources/debugprobe
rm -rf build && mkdir build && cd build
cmake ..
cmake --build . -j
```
生成された `build/debugprobe.uf2` を BOOTSEL モードの Pico にコピーすると、CMSIS-DAP デバッガとして利用できます。

## 実機テストの実行
1. デバッグプローブ（Picoprobe 等）とターゲット（W5500-EVB-Pico / W55RP20-EVB-Pico）の SWDIO/SWCLK/GND/3V3 を配線。
2. `.cargo/config.toml` で runner を設定（例: `probe-rs run --chip RP2040`）。
3. クロスビルド・テストコマンド
   ```bash
   cargo check -p rp2040-hw-tests --target thumbv6m-none-eabi
   cargo test -p rp2040-hw-tests --features hardware-tests --target thumbv6m-none-eabi
   ```
   ※ `probe-rs` がデバイスを検出できない場合は `probe-rs list` で接続状況を確認。

## UF2 書き込みの簡易手順
デバッグプローブが無い場合は BOOTSEL モードで `.uf2` をコピーして動作確認できます。

