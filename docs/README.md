# Edamame のドキュメント

Edamame MVP は、ライブ HLS のセグメントを受け取る `edamame-core` と、
セグメントを事前に取得して配信する `edamame-node` で動きます。

外部のソフトウェアから使う場合は、まず次のドキュメントを見てください。

- [API リファレンス](./api.md): HTTP エンドポイント、JSON、ステータスコード
- [ローカルで動かす](./running.md): 起動例と `curl` での確認手順

## MVP の前提

- `edamame-core` は origin と controller の役割をまとめたプロセスです。
- origin は、登録されたセグメントの正規コピーを持ちます。
- controller は、cache node の状態を見て playlist の向き先を決めます。
- `edamame-node` は単純な cache node です。全体の負荷分散や playlist 検証はしません。
- Publisher はセグメントだけを PUT します。master playlist と media playlist は Edamame が生成します。
- 認証はありません。ローカルまたは private network で使う前提です。

## 扱うデータ

- Segment は `channel_id`、`rendition`、`sequence` で識別します。
- Rendition は segment URL の path にある `rendition` から決まります。
- Segment のファイル名は URL 用です。識別には `sequence` を使います。
- Cache node は `COLD`、`ACTIVE`、`DRAINING`、`DEAD` のどれかの状態になります。
- Live window は、再生に必要な最新セグメントの範囲です。
- Retention window は、live edge からどこまで古いセグメントを残すかを表します。

## Cache node の状態

- `COLD`: 参加直後、または必要なセグメントが足りない状態です。事前取得が終わるまで client 向け playlist には使いません。
- `ACTIVE`: 必要なセグメントを持っていて、health が `ok` で、負荷も高くない状態です。client 向け playlist に使えます。
- `DRAINING`: 負荷が高いため、新しい client 向け playlist には使わない状態です。負荷が戻り、health が `ok` なら `ACTIVE` に戻ります。
- `DEAD`: heartbeat が一定時間届いていない状態です。playlist には使いません。heartbeat が再開すると `COLD` に戻ります。
