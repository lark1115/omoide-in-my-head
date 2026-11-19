# omoide-in-my-head

LLM のエージェントのための、長期記憶の装置。実行ファイルは `omoide`。

エージェントは、決まりも、決めたことも、記録としては持っている。それでも、要る場面でその記録が出てこない。`omoide` は、記録を、要る場面に届けるために作る。

- 強くない Linux で、常駐するプロセスなしで動く。
- 保存の既定は SQLite（Cloudflare D1 でも動く書き方。Turso にも対応する）。
- 判定の層（型つきの問いに確率で答えるモデルなど）は差し替えられ、無くても動く。

## 状態

要求定義の段。動くコードは、まだ無い。

- 要求: [docs/requirements.md](docs/requirements.md)
- LLM が組み込む時の手引き: [AGENTS.md](AGENTS.md)（実装と一緒に書く）

## 進め方

要求（docs）→ システムの要件（req）→ テスト、の順に記録し、[miwa](https://github.com/lark1115/miwa) で食い違いを検出する。
