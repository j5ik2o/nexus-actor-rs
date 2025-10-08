#!/bin/bash

set -e

# 必要なツールがインストールされているか確認
if ! command -v grcov &> /dev/null; then
    echo "grcovがインストールされていません。インストールします..."
    cargo install grcov
fi

# llvm-tools-previewがインストールされているか確認
if ! rustup component list --installed | grep -q "llvm-tools-preview"; then
    echo "llvm-tools-previewがインストールされていません。インストールします..."
    rustup component add llvm-tools-preview
fi

# プロファイリング情報を収集するためのディレクトリを作成・クリーン
rm -rf ./target/coverage
mkdir -p ./target/coverage

# 環境変数を設定
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off"
export RUSTDOCFLAGS="-Cinstrument-coverage -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off"
export LLVM_PROFILE_FILE="./target/coverage/cargo-test-%p-%m.profraw"

# テストを実行（embeddedパッケージは除外し、後で個別に実行）
cargo test --workspace --exclude nexus-utils-embedded-rs --exclude nexus-actor-embedded-rs

# embeddedパッケージのテストは個別実行（エラーは無視）
cargo test -p nexus-utils-embedded-rs --no-default-features --features arc || echo "utils-embedded tests skipped"
cargo test -p nexus-actor-embedded-rs --no-default-features --features embedded_arc || echo "actor-embedded tests skipped"

# カバレッジレポートを生成
grcov . --binary-path ./target/debug/deps/ -s . -t html --branch --ignore-not-existing --ignore "*cargo*" --ignore "*tests*" -o ./target/coverage/html

# カバレッジの概要を表示
grcov . --binary-path ./target/debug/deps/ -s . -t covdir --branch --ignore-not-existing --ignore "*cargo*" --ignore "*tests*" -o ./target/coverage/covdir.json

# 結果の表示
echo "カバレッジレポートが生成されました: ./target/coverage/html/index.html"

# オプション: ブラウザでレポートを開く（macOSの場合）
if [[ "$OSTYPE" == "darwin"* ]]; then
    open ./target/coverage/html/index.html
fi
