# TODO — 課題一覧

## 凡例
- 🔴 **バグ / データ破壊** — 放置するとランタイムエラーまたはデータ不整合
- 🟡 **要件ずれ** — 実装が仕様と食い違っている
- 🟠 **設計の問題** — 動くが後で困る
- 🟢 **改善** — 対応しなくても致命的ではない

## ステータスまとめ

| ID | 優先度 | 内容 | 状態 |
|----|--------|------|------|
| T-01 | 🔴 高 | `machine_items.name NOT NULL` 制約 vs 要件「名前不要」 | ✅ 対応済み (`5dbe87e`) |
| T-02 | 🟡 中 | `analyze_image.py` の不要な `name` 出力 | ✅ 対応済み (`7e23eba`) |
| T-03 | 🔴 高 | `update()` RETURNING で投票数が常に 0 | ✅ 対応済み (`32897aa`) |
| T-04 | 🟡 中 | `renderItemsList` の `item.name` 無条件依存 | ✅ 対応済み (`fe054d1`) |
| T-05 | 🟠 中 | OCR フィルター条件の `name` 依存 | ✅ 対応済み (`5dbe87e`) |
| T-06 | 🟠 低 | analyzer エラーが HTTP 502 になる | ✅ 対応済み (`fe054d1`) |
| T-07 | 🟢 低 | INSERT に `name` カラムが残存 | ✅ 現状維持で問題なし(下記参照) |
| T-08 | 🟢 低 | 未使用 import `ImageEnhance` | ✅ 対応済み (`7e23eba`) |
| T-09 | 🟢 低 | `file_size: i32` の潜在オーバーフロー | ⏳ 未対応 → [Issue #1](https://github.com/3211133/vending-map/issues/1) |

---

## 対応済み項目の詳細

### T-01: `machine_items.name` を NULL 許可に変更 ✅
- `migrations/007_alter_machine_items_name_nullable.sql` で `NOT NULL` を解除
- `models/image.rs` の `name` を `Option<String>` に変更(3箇所)
- `handlers/images.rs` の `"不明"` フォールバックを削除
- テスト: `backend/src/models/image.rs` の `t01_insert_item_name_is_option_after_fix`

### T-02: `analyze_image.py` から `name` 出力を削除 ✅
- `clean_name()` と名前抽出用 OCR を削除。出力は `{"price_yen": ..., "temperature": ...}` のみ
- `ocr_region()` は価格抽出に必要なため残置
- テスト: `scripts/test_analyze_image.py`

### T-03: `update()` 後の投票数を保持 ✅
- `vending_machine_repo::update()` が更新後に `get_by_id()` で再取得する方式に変更
- `create()` の `0::BIGINT` は新規作成直後のため仕様通り(変更なし)
- テスト: `backend/tests/vote_count_after_update.rs`(実 DB 統合テスト)

### T-04: `renderItemsList` の null 安全化 ✅
- `frontend/js/app.js` で `item.name` が null/undefined の場合に名前部分を省略

### T-05: スロットフィルターを temperature 対応に変更 ✅
- `handlers/images.rs` のフィルターを `price_yen.is_some() || temperature != "unknown"` に変更
- テスト: `backend/src/models/image.rs` の `t05_slot_filter_*`

### T-06: analyzer エラーを HTTP 500 に変更 ✅
- `image_analysis.rs` のエラーを `AppError::internal()` に変更(`error.rs` にコンストラクタ追加)

### T-07: INSERT の `name` カラム 🟢 現状維持
- T-01 で NULL 許可になったため `Option<String>` の bind で正常動作する
- `name` カラムを完全削除する場合のみ `image_repo.rs:54` の INSERT 文の変更が必要

### T-08: `ImageEnhance` import 削除 ✅

---

## 未対応項目

### 🟢 T-09: `file_size: i32` の潜在的オーバーフロー → [Issue #1](https://github.com/3211133/vending-map/issues/1)

**ファイル:** `backend/src/models/image.rs:12`, `handlers/images.rs:78`

`usize as i32` のキャストは silent truncation。ファイルサイズ上限 10MB の現状では安全だが、上限を 2GB 超に変更する場合は要対応。詳細は Issue #1 を参照。

---

## 残課題(2026-07-07 時点)

T-01〜T-08 の完了後に残っている、コード修正以外も含む課題。

### 🟠 マイグレーション適用管理が無い
`docker-entrypoint-initdb.d` によるマイグレーション適用は postgres_data ボリュームの**初回作成時のみ**実行される。既存ボリュームには 007 以降が自動適用されないため、手動適用が必要:

```
docker exec vending-map-postgres-1 psql -U vending -d vending_map \
  -f /docker-entrypoint-initdb.d/007_alter_machine_items_name_nullable.sql
```

恒久対応としては `sqlx migrate` などの適用管理ツールの導入を検討する。

### 🟠 CI 未整備
GitHub Actions が無く、`cargo test` / `cargo clippy -D warnings` / `scripts/test_analyze_image.py` はローカル実行のみ。統合テストは PostGIS が必要なため、CI では services コンテナ(postgis/postgis:16-3.4)を立てる構成になる。

### 🟡 ブラウザ/E2E での動作確認が未実施
- T-04 の修正(名前なしアイテムが「⚪ ¥130」と表示される)はコードレビューと単体テストのみで、ブラウザでの目視確認は未実施
- `tests/e2e/specs/smoke.spec.ts`(Playwright)も直近の変更に対して未実行

### 🟢 `error.rs` の `Upstream` バリアントが未使用
T-06 で `image_analysis.rs` が `AppError::internal()` に移行した結果、`Upstream` / `upstream()`(`error.rs:21,66`)はどこからも使われていない。Overpass API クライアント等で将来使う予定がなければ削除候補。

### 🟢 `image.rs` のテストコメントが修正前の記述のまま
`t05_slot_filter_drops_temperature_only_slot` のコメントが修正前のフィルター条件を「現在の条件」と記述している。実際の `handlers/images.rs:93` は修正済みのため、「修正前の条件の再現」と読み替える必要がある。

### ℹ️ OSM 元データはリポジトリ非包含(運用メモ)
`data/japan-latest.osm.pbf`(2.3GB)と `data/kanto-latest.osm.pbf`(439MB)は GitHub の 100MB 制限のため git 履歴から除去済み(.gitignore 登録済み)。必要な場合は [geofabrik](https://download.geofabrik.de/asia/japan.html) から再取得する。抽出済みの `*-vending.osm.pbf`(小サイズ)はリポジトリに含まれる。
