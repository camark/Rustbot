# RustBot

🐈 超軽量パーソナル AI アシスタントフレームワーク（Rust 実装）

これは [nanobot](https://github.com/HKUDS/nanobot) の完全な Rust 書き直しで、以下のために設計されています：

- **高性能**：Python より 10-100 倍高速
- **低メモリ**：GC なし、最小フットプリント
- **シングルバイナリ**：簡単なデプロイ、依存関係なし
- **型安全性**：コンパイル時の保証

## プロジェクト構造

```
RustBot/
├── Cargo.toml              # ワークスペースルート
├── crates/
│   ├── nanobot-config/     # 設定システム
│   ├── nanobot-providers/  # LLM プロバイダー
│   ├── nanobot-bus/        # メッセージバス
│   ├── nanobot-core/       # エージェントエンジン
│   └── nanobot-cli/        # CLI インターフェース
└── docs/
```

## クイックスタート

### ビルド

```bash
cargo build --release
```

### 初期化

```bash
./target/release/rustbot onboard
```

### チャット

```bash
./target/release/rustbot agent -m "こんにちは！"
```

### インタラクティブモード

```bash
./target/release/rustbot agent
```

### ステータス

```bash
./target/release/rustbot status
```

## 設定

設定ファイル：`~/.nanobot/config.json`

```json
{
  "providers": {
    "openrouter": {
      "apiKey": "sk-or-v1-xxx"
    }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-opus-4-5",
      "provider": "openrouter"
    }
  }
}
```

## サポートされている LLM プロバイダー

| プロバイダー | ステータス |
|----------|--------|
| OpenRouter | ✅ |
| Anthropic | ✅ |
| OpenAI | ✅ |
| DeepSeek | ✅ |
| Azure OpenAI | ✅ |
| Ollama (ローカル) | ✅ |
| vLLM (ローカル) | ✅ |
| Groq | ✅ |
| Moonshot | ✅ |
| Gemini | ✅ |
| Zhipu | ✅ |
| DashScope | ✅ |

## 機能

### ✅ 完了

- **コアインフラ**：設定システム、プロバイダーレジストリ、メッセージバス、CLI フレームワーク
- **ツール実装**：シェル実行、ファイルシステムツール、ウェブ検索、ウェブフェッチ
- **メモリとセッション**：セッション永続化、メモリ統合、コンテキストウィンドウ管理
- **チャンネル統合**：Telegram、Discord、Feishu チャンネルコネクタ
- **サービスシステム**：Cron サービス、ハートビートサービス、OpenAI 互換 API サーバー
- **高度な機能**：MCP (モデルコンテキストプロトコル)、サブエージェントシステム、スキルシステム

## 開発

### 前提条件

- Rust 1.75+ (stable)
- Tokio ランタイム

> **Ubuntu ユーザー向け：protoc のインストール**
>
> ビルド中に protoc 関連のエラーが発生した場合は、Protocol Buffers コンパイラーをインストールしてください：
>
> ```bash
> # Ubuntu/Debian
> sudo apt-get update && sudo apt-get install -y protobuf-compiler
>
> # 検証
> protoc --version
> ```

### テストの実行

```bash
cargo test
```

### Clippy の実行

```bash
cargo clippy --all-targets
```

### コードのフォーマット

```bash
cargo fmt --all
```

## CLI コマンド

| コマンド | 説明 |
|------|------|
| `rustbot onboard` | 設定を初期化 |
| `rustbot agent -m "メッセージ"` | 単一メッセージを送信 |
| `rustbot agent` | インタラクティブモード |
| `rustbot api --port 8900` | API サーバーを起動 |
| `rustbot cron list` | クロンジョブを一覧 |
| `rustbot channels login <チャンネル>` | チャンネルにログイン |
| `rustbot channels status` | チャンネルステータスを表示 |
| `rustbot channels start <チャンネル>` | チャンネルを開始 |
| `rustbot channels stop <チャンネル>` | チャンネルを停止 |
| `rustbot mcp list` | MCP サーバーを一覧 |

## ライセンス

MIT ライセンス - 元の nanobot プロジェクトと同じ。

## 謝辞

このプロジェクトは、超軽量 Python AI アシスタントフレームワークである [nanobot](https://github.com/HKUDS/nanobot) からインスピレーションを受けました。
