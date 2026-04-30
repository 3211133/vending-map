# TODO — 課題一覧

## 凡例
- 🔴 **バグ / データ破壊** — 放置するとランタイムエラーまたはデータ不整合
- 🟡 **要件ずれ** — 実装が仕様と食い違っている
- 🟠 **設計の問題** — 動くが後で困る
- 🟢 **改善** — 対応しなくても致命的ではない

---

## 🔴 T-01: `machine_items.name NOT NULL` 制約 vs 要件「名前不要」

**ファイル:**
- `migrations/006_create_machine_items.sql:5` — `name TEXT NOT NULL`
- `backend/src/models/image.rs:25` — `pub name: String`
- `backend/src/models/image.rs:33` — `pub name: String`（ItemResponse）
- `backend/src/models/image.rs:70` — `pub name: String`（InsertItemInput）
- `backend/src/handlers/images.rs:97` — `name: s.name.unwrap_or_else(|| "不明".to_string())`

**問題:** ユーザー要件は「外観・価格・冷温のみ」で商品名は不要。しかし DB スキーマの `name` は `NOT NULL` のため、Python が `name: null` を返すと INSERT が失敗するか、`"不明"` というダミー値が入る。

**修正:**
1. `migrations/007_alter_machine_items_name_nullable.sql` を作成し `name` を NULL 許可に変更
2. `models/image.rs` の `name: String` を `name: Option<String>` に変更（3箇所）
3. `handlers/images.rs` の `"不明"` フォールバックを削除し `name: s.name` に変更
4. `InsertItemInput.name` を `Option<String>` に変更し `image_repo.rs` の bind も対応

**テスト:** `tests/unit/image_name_nullable_test.rs` — コンパイル時型チェックで検出可能

---

## 🟡 T-02: `analyze_image.py` が不要な `name` フィールドを出力している

**ファイル:** `scripts/analyze_image.py:167,173`

**問題:** 要件は価格と冷温のみだが、スクリプトは `name` フィールドを含む JSON を返す。`clean_name()` と OCR による名前抽出が無駄に処理時間を消費している（tesseract 呼び出しは 200–500 ms）。

**修正:**
- `clean_name()` 関数を削除
- `ocr_region()` 呼び出しを削除（価格抽出の OCR も不要なら削除）
- スロットから `name` キーを削除し `{"price_yen": ..., "temperature": ...}` のみ出力
- docstring の Output format を更新

**テスト:** `tests/unit/test_analyze_image.py` — JSON 出力に `name` キーが含まれないことを検証

---

## 🔴 T-03: `update()` RETURNING で投票数が常に 0 になる

**ファイル:** `backend/src/repositories/vending_machine_repo.rs:208-209`

```sql
0::BIGINT AS exists_count,
0::BIGINT AS removed_count,
```

**問題:** UPDATE 後の RETURNING 句でハードコードの `0` を返しているため、`update()` を呼んだ直後は投票数が 0 になって返る。`create()` も同様（行 155-156）。

- `create()` は新規作成直後なので 0 は正しい → **問題なし**
- `update()` は既存レコードの更新なので 0 は**バグ** — API レスポンスで投票数が消える

**修正:** `update()` の RETURNING を `get_by_id()` と同様に LEFT JOIN 形式に変更するか、更新後に `get_by_id()` を呼び出すように変更する。

**テスト:** `tests/unit/vote_count_after_update_test.rs` — `update()` 後に exists_count が保持されることを検証

---

## 🟡 T-04: `renderItemsList` が `item.name` に無条件依存

**ファイル:** `frontend/js/app.js:895`

```js
li.textContent = `${icon} ${item.name}${price}`;
```

**問題:** T-01/T-02 の修正後、API の `items[].name` は `null` になる可能性がある。現状のコードは `null` をそのまま文字列結合するため「⚪ null ¥130」のように表示される。

**修正:** `item.name` が null/undefined の場合に名前部分を省略するよう変更。

```js
const name = item.name ? `${item.name} ` : "";
li.textContent = `${icon} ${name}${price}`.trim();
```

**テスト:** ブラウザ／Playwright — 名前なし・価格なしのアイテムで「⚪ ¥130」と表示されることを確認

---

## 🟠 T-05: OCR フィルター条件が `name` に依存している

**ファイル:** `backend/src/handlers/images.rs:93`

```rust
.filter(|s| s.name.is_some() || s.price_yen.is_some())
```

**問題:** `name` を廃止した後、このフィルターは `s.price_yen.is_some()` のみになる。すると **temperature だけ取れたスロット（price_yen = null）が捨てられる**。冷温情報を保存するという要件を満たせない。

**修正:** フィルター条件を `price_yen.is_some() || temperature != "unknown"` に変更。

**テスト:** `tests/unit/slot_filter_test.rs` — temperature のみのスロットが保持されることを検証

---

## 🟠 T-06: `image_analysis.rs` のエラーが HTTP 502 Bad Gateway になる

**ファイル:** `backend/src/services/image_analysis.rs:35`
**ファイル:** `backend/src/error.rs:86` — `Upstream => StatusCode::BAD_GATEWAY`

**問題:** Python スクリプトが存在しない、実行権限がない、依存ライブラリが不足している場合に HTTP 502 を返す。502 は「上流サーバーへの接触失敗」を意味するが、これはサーバー内部の処理失敗なので 500 Internal Server Error が適切。

**修正:** `image_analysis.rs` のエラーを `AppError::upstream()` から `AppError::database()` に変更（または新しい `AppError::internal()` コンストラクタを追加）。

**テスト:** コンパイル時には検出不可。統合テストまたはコードレビューで対応。

---

## 🟢 T-07: `machine_items` INSERT で `name` カラムを含んでいる

**ファイル:** `backend/src/repositories/image_repo.rs:54`

```rust
QueryBuilder::new("INSERT INTO machine_items (id, image_id, machine_id, name, price_yen, temperature) ")
```

**問題:** T-01 で `name` を NULL 許可にした後も、INSERT 文は `name` カラムを含めている。T-01 の修正と合わせて、`name` が `Option<String>` になれば bind で `None` を渡せるため動作はする。ただし、将来的に `name` カラムを完全削除するなら、ここも合わせて変更が必要。

**現時点:** T-01 の修正後は動作に問題なし（NULL 値を INSERT 可能になるため）。

---

## 🟢 T-08: `ImageEnhance` が import されているが未使用

**ファイル:** `scripts/analyze_image.py:18`

```python
from PIL import Image, ImageEnhance, ImageFilter
```

**問題:** `ImageEnhance` が import されているが使用されていない。無害だが不要。

**修正:** `ImageEnhance` を import から削除。

---

## 🟢 T-09: `file_size: i32` の潜在的オーバーフロー

**ファイル:** `backend/src/models/image.rs:12`, `handlers/images.rs:78`

```rust
file_size: bytes.len() as i32,
```

**問題:** `usize as i32` のキャストは Rust では silent truncation（パニックなし）。ファイルサイズ上限が 10 MB = 10,485,760 で `i32::MAX` は 2,147,483,647 なので現状は問題ない。しかし上限を 2 GB 以上に変更した場合に溢れる。

**現時点:** 10 MB 制限がある限り安全。

---

## まとめ: 優先順位

| ID | 優先度 | 難易度 | テスト可否 |
|----|--------|--------|-----------|
| T-01 | 🔴 高 | 小 | コンパイル時 |
| T-02 | 🟡 中 | 小 | Python 単体テスト |
| T-03 | 🔴 高 | 小 | Rust 単体テスト（DB 要） |
| T-04 | 🟡 中 | 小 | ブラウザ目視 |
| T-05 | 🟠 中 | 小 | Rust 単体テスト |
| T-06 | 🟠 低 | 小 | コードレビュー |
| T-07 | 🟢 低 | 小 | — |
| T-08 | 🟢 低 | 小 | — |
| T-09 | 🟢 低 | 低 | — |
