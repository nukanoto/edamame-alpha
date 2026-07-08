# API リファレンス

このドキュメントは、外部のソフトウェアから Edamame Alpha を操作するための最小限の HTTP 仕様です。

## ベース URL

- Core: `http://127.0.0.1:9000`
- Node: `http://127.0.0.1:9001`

実際の URL は、起動時の `--bind` と `--core-url` で決まります。

## 共通ルール

- エラー応答の本文は JSON ではなく、プレーンテキストです。
- セグメントの本文は任意のバイト列です。
- セグメントのファイル名は `101.ts` のように、数値のシーケンスと拡張子を含めます。
- セグメントは `channel_id`、`rendition`、`sequence` の組で識別します。
- HLS プレイリストの Content-Type は `application/vnd.apple.mpegurl` です。
- セグメント配信の Content-Type は `application/octet-stream` です。
- 認証はありません。公開インターネットに直接出さない前提です。

## Playlist と cache node の扱い

- Publisher は master playlist や media playlist を PUT しません。
- Edamame は登録済みのセグメントから playlist を生成します。
- 生成した playlist が欠落セグメントを参照する場合、Edamame はその playlist を返しません。
- 複数 rendition の master playlist は、全 rendition の live window がそろっている場合だけ返します。
- client 向けの media playlist では、すべてのセグメント URL が 1 つの ACTIVE node を指します。
- ACTIVE node がない場合、core は origin URL に逃がさず `503 Service Unavailable` を返します。
- COLD、DRAINING、DEAD の node は、新しい client 向け playlist には使いません。
- DEAD になった node が heartbeat を再開した場合、その node は COLD に戻ります。

## Core API

### セグメントを登録する

```http
PUT /segments/{channel_id}/{rendition}/{filename}?bandwidth={bandwidth}&resolution={resolution}
```

Publisher が core に正規セグメントを登録します。`bandwidth` と `resolution` は省略できます。

例:

```bash
curl -i -X PUT \
  --data-binary @101.ts \
  'http://127.0.0.1:9000/segments/live/720p/101.ts?bandwidth=3000000&resolution=1280x720'
```

ステータス:

- `201 Created`: 新しいセグメントを保存した
- `200 OK`: 同じ識別子と同じバイト列がすでに保存されている
- `400 Bad Request`: 識別子、ファイル名、シーケンス、メタデータが不正
- `409 Conflict`: 同じ識別子に別のバイト列がある、または同じ rendition に別のメタデータがある

### 正規セグメントを取得する

```http
GET /segments/{channel_id}/{rendition}/{filename}
```

Node が事前取得をするときに、core から正規セグメントを取得します。

ステータス:

- `200 OK`: セグメントのバイト列を返す
- `404 Not Found`: セグメントが存在しない

### Master playlist を取得する

```http
GET /channels/{channel_id}/master.m3u8
```

Edamame が生成した HLS master playlist を返します。

ステータス:

- `200 OK`: master playlist を返す
- `404 Not Found`: channel が存在しない、または playlist を生成できない
- `422 Unprocessable Entity`: rendition 間の live window がそろっていない
- `503 Service Unavailable`: 利用できる ACTIVE node がない

### Media playlist を取得する

```http
GET /channels/{channel_id}/{rendition}.m3u8
```

Edamame が生成した HLS media playlist を返します。playlist 内のセグメント URL は、選ばれた ACTIVE node を指します。

ステータス:

- `200 OK`: media playlist を返す
- `404 Not Found`: rendition が存在しない、または playlist を生成できない
- `422 Unprocessable Entity`: playlist が未保存または欠落したセグメントを参照している
- `503 Service Unavailable`: 利用できる ACTIVE node がない

### Node を登録する

```http
PUT /nodes/{node_id}
Content-Type: application/json
```

Node を core に登録します。登録済みの node に対しては、公開 URL を更新します。

リクエスト:

```json
{
  "base_url": "http://127.0.0.1:9001",
  "capacity_hint": 100
}
```

`capacity_hint` は省略できます。

ステータス:

- `201 Created`: 新しい COLD node を登録した
- `200 OK`: 登録済みの node を更新した

### Heartbeat を送る

```http
POST /nodes/{node_id}/heartbeat
Content-Type: application/json
```

Node の状態、負荷、保持しているセグメントを core に送ります。core は現在の node state と、事前取得が必要なセグメントを返します。

リクエスト:

```json
{
  "health": "ok",
  "load_score": 0.42,
  "retained_segments": [
    { "channel_id": "live", "rendition": "720p", "sequence": 101 }
  ]
}
```

レスポンス:

```json
{
  "state": "ACTIVE",
  "prewarm": [
    { "channel_id": "live", "rendition": "720p", "filename": "102.ts" }
  ]
}
```

値:

- `health`: `ok`, `bad`
- `state`: `COLD`: 参加直後、または必要なセグメントが足りない。playlist には使わない
- `state`: `ACTIVE`: 必要なセグメントを持ち、health が `ok` で、負荷も高くない。playlist に使える
- `state`: `DRAINING`: 負荷が高い。新しい playlist には使わず、回復したら `ACTIVE` に戻る
- `state`: `DEAD`: heartbeat が一定時間届いていない。生きているか分からないので playlist には使われない。heartbeat が再開したら `COLD` に戻る

ステータス:

- `200 OK`: heartbeat を受け付けた
- `400 Bad Request`: retained segment の識別子が不正
- `404 Not Found`: node が登録されていない

## Node API

### セグメントを事前取得する

```http
PUT /prewarm/{channel_id}/{rendition}/{filename}
Content-Type: application/json
```

Node に origin URL からセグメントを取得させ、ローカルに保持させます。

リクエスト:

```json
{
  "origin_url": "http://127.0.0.1:9000/segments/live/720p/101.ts"
}
```

ステータス:

- `202 Accepted`: 事前取得が完了した、またはすでに保持している
- `400 Bad Request`: ファイル名や識別子が不正
- `502 Bad Gateway`: origin URL から取得できない

### セグメントを配信する

```http
GET /segments/{channel_id}/{rendition}/{filename}
```

Node が保持しているセグメントを返します。

ステータス:

- `200 OK`: セグメントのバイト列を返す
- `400 Bad Request`: ファイル名や識別子が不正
- `404 Not Found`: この node がセグメントを保持していない
