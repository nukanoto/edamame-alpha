# ローカルで動かす

外部のソフトウェアや手元の `curl` から Edamame MVP を試すための最小手順です。

## ビルドする

```bash
cargo build
```

## Core を起動する

```bash
cargo run --bin edamame-core -- \
  --bind 127.0.0.1:9000 \
  --storage-dir .edamame/core \
  --live-window 6 \
  --retention-window 12 \
  --heartbeat-timeout-ms 10000 \
  --overload-threshold 0.80
```

## Node を起動する

別のターミナルで実行します。

```bash
cargo run --bin edamame-node -- \
  --node-id cache-a \
  --bind 127.0.0.1:9001 \
  --core-url http://127.0.0.1:9000 \
  --cache-dir .edamame/cache-a \
  --heartbeat-interval-ms 1000
```

`edamame-node` は起動時に core へ登録し、その後 heartbeat を送ります。起動直後の登録に失敗しても、プロセスはそのまま動きます。

## 動作を確認する

セグメントファイルを作ります。

```bash
printf 'segment-101' > /tmp/101.ts
```

core にセグメントを登録します。

```bash
curl -i -X PUT \
  --data-binary @/tmp/101.ts \
  'http://127.0.0.1:9000/segments/live/720p/101.ts?bandwidth=3000000&resolution=1280x720'
```

node にセグメントを事前取得させます。

```bash
curl -i -X PUT \
  -H 'Content-Type: application/json' \
  -d '{"origin_url":"http://127.0.0.1:9000/segments/live/720p/101.ts"}' \
  'http://127.0.0.1:9001/prewarm/live/720p/101.ts'
```

node からセグメントを取得します。

```bash
curl -i 'http://127.0.0.1:9001/segments/live/720p/101.ts'
```

node が ACTIVE になった後で playlist を取得します。

```bash
curl -i 'http://127.0.0.1:9000/channels/live/master.m3u8'
curl -i 'http://127.0.0.1:9000/channels/live/720p.m3u8'
```

## CLI 引数

### `edamame-core`

- `--bind`: core の HTTP server が待ち受けるアドレス
- `--storage-dir`: 正規セグメントの保存先
- `--live-window`: HLS live window として扱うセグメント数
- `--retention-window`: eviction 前に保持する live edge からの距離。`--live-window` 以上が必要
- `--heartbeat-timeout-ms`: node を `DEAD` と見なすまでの heartbeat 待ち時間
- `--overload-threshold`: node を過負荷扱いにする `load_score` のしきい値。`0.0` から `1.0`

### `edamame-node`

- `--node-id`: core に登録する node 識別子
- `--bind`: node の HTTP server が待ち受けるアドレス
- `--core-url`: core のベース URL
- `--cache-dir`: node が保持セグメントを保存するディレクトリ
- `--heartbeat-interval-ms`: heartbeat の送信間隔
