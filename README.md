# Claude Usage

Windowsのタスクトレイに常駐し、`claude -p "/usage"` の出力をフライアウトUIで表示するTauri v2製デスクトップアプリです。

## 開発

```powershell
npm install
npm run tauri dev
```

## ビルド

```powershell
npm run tauri build
```

成功すると以下が生成されます。

- `src-tauri/target/release/bundle/nsis/*.exe` — NSISインストーラー
- `src-tauri/target/release/tauri-app.exe` — ポータブル版（インストール不要の単体実行ファイル）

## リリース手順

GitHub Actions（`.github/workflows/release.yml`）がタグ push (`v*`) をトリガーに自動ビルドし、GitHub Releasesにドラフトを作成します。内容を確認後、手動でPublishしてください。

初回セットアップとして、以下をリポジトリ管理者が手動で行う必要があります。

### 1. アップデート署名鍵の生成

自動アップデート機能（`tauri-plugin-updater`）は、配布物の改ざん検知のため署名鍵ペアを使用します。以下のコマンドで鍵ペアを生成します。

```powershell
npm run tauri signer generate -- -w $env:USERPROFILE\.tauri\easy-claude-usage.key
```

- 対話式でパスワードの設定を求められます。忘れずに控えてください。
- `easy-claude-usage.key`（秘密鍵）と `easy-claude-usage.key.pub`（公開鍵）が生成されます。
- **秘密鍵は絶対にリポジトリにコミットしないでください**（`.gitignore` で `*.key` を除外済みです）。

### 2. 公開鍵を `tauri.conf.json` に反映

`easy-claude-usage.key.pub` の中身（ファイルパスではなく、テキストの中身そのもの）を、`src-tauri/tauri.conf.json` の `plugins.updater.pubkey` に貼り付けます。

```json
"plugins": {
  "updater": {
    "pubkey": "ここに .key.pub の中身を貼り付ける"
  }
}
```

プレースホルダー（`__TAURI_UPDATER_PUBKEY_PLACEHOLDER__`）のままだと、ビルド時の署名生成やクライアント側のアップデート確認が正しく動作しません。

### 3. GitHub Secretsへの登録

秘密鍵とパスワードをGitHub Actionsから参照できるよう、リポジトリのSecretsに登録します。

GitHub UI（Settings → Secrets and variables → Actions → New repository secret）、または `gh` CLIで登録できます。

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY --body (Get-Content -Raw $env:USERPROFILE\.tauri\easy-claude-usage.key)
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body "生成時に設定したパスワード"
```

登録するSecret名:

| Secret名 | 内容 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `easy-claude-usage.key` の中身 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 鍵生成時に設定したパスワード |

### 4. バージョンを上げてタグをpush

以下3ファイルの `version` を一致させてからコミットします（ズレているとupdaterが新バージョンを正しく検知できません）。

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

```powershell
git tag vX.Y.Z
git push origin main --tags
```

### 5. ドラフトReleaseをPublish

GitHub Actionsのビルドが完了すると、GitHub Releasesにドラフトが作成されます。内容（アセット一覧: NSISインストーラー・ポータブルexe・`latest.json`）を確認し、問題なければPublishしてください。Publish後、既存クライアントが自動アップデート確認（起動時・トレイメニューの「アップデートを確認」・ヘッダーの更新ボタン）で新バージョンを検知できるようになります。

## 注意事項

- 自動アップデートはNSISインストーラー経由でインストールした場合のみ動作します。ポータブルexeは配布形態としてのみ提供され、自動更新の対象外です。
- 対応OSはWindowsのみです。

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
