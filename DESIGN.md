# vending-map 設計ドキュメント

## 概要

OpenStreetMap (OSM) の自動販売機データを地図上に可視化し、ユーザーがクリック操作で新規自販機を登録・投票できるWebアプリケーション。

## 技術スタック

| 層 | 技術 |
|---|---|
| フロントエンド | HTML + Leaflet.js (CDN, ビルド不要) |
| バックエンド | Rust + Axum 0.7 |
| DB | PostgreSQL 16 + PostGIS |
| インフラ | Docker Compose (nginx + backend + postgres) |

## アーキテクチャ

```
ブラウザ (Leaflet.js)
    ↓ HTTP
nginx :80
  ├── /        → frontend/ 静的ファイル
  └── /api/    → backend :8080 (Axum)
                     ↓
                PostgreSQL+PostGIS :5432
                     ↑ (インポート時)
           Overpass API (overpass-api.de)
```

## ディレクトリ構成

```
vending-map/
  docker-compose.yml
  .env.example
  .gitignore
  DESIGN.md
  docker/
    backend.Dockerfile       # マルチステージビルド
    nginx.conf               # リバースプロキシ + 静的配信
  migrations/                # 連番SQLファイル (Postgres初期化時に自動実行)
    001_create_extensions.sql
    002_create_vending_machines.sql
    003_create_machine_votes.sql
    004_create_import_logs.sql
  backend/
    Cargo.toml
    src/
      main.rs                # サーバー起動・ルーター定義
      config.rs              # 環境変数 → AppConfig
      error.rs               # AppError enum + IntoResponse
      models/
        mod.rs
        vending_machine.rs   # Row型・リクエスト型
        vote.rs              # Vote型・VoteSummary
        import_log.rs        # ImportLog型
      handlers/
        mod.rs               # to_geojson_feature ヘルパー
        health.rs
        vending_machine.rs   # GET/POST/PATCH/DELETE
        votes.rs             # POST /:id/votes
        import.rs            # POST /import/osm, GET /import/logs
      repositories/
        mod.rs
        vending_machine_repo.rs  # DB操作・PostGIS bbox クエリ
        vote_repo.rs             # 投票・ステータス自動更新
        import_log_repo.rs       # インポートログ
      services/
        mod.rs
        overpass.rs          # Overpass API HTTPクライアント
        osm_importer.rs      # インポート全体フロー
  frontend/
    index.html               # CDN読み込み・DOM構造
    css/
      style.css              # フルスクリーン地図・サイドバー・投票UI
    js/
      app.js                 # 全機能を1ファイルに実装
```

## データベーススキーマ

### vending_machines

```sql
CREATE TABLE vending_machines (
    id            UUID         PRIMARY KEY DEFAULT uuid_generate_v4(),
    osm_id        BIGINT       UNIQUE,
    geom          GEOMETRY(Point, 4326) NOT NULL,  -- PostGIS WGS84
    name          TEXT,
    vending_type  TEXT         NOT NULL DEFAULT 'drinks',
    operator      TEXT,
    opening_hours TEXT,
    source        TEXT         NOT NULL DEFAULT 'user',  -- 'osm' | 'user'
    osm_tags      JSONB,                                 -- OSM生タグ保存
    is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
    trust_score   SMALLINT     NOT NULL DEFAULT 50,      -- 0-100
    status        TEXT         NOT NULL DEFAULT 'unverified'
                               CHECK (status IN ('active','suspected_removed','removed','unverified')),
    confirmed_at  TIMESTAMPTZ,
    last_verified DATE,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- GiST インデックスでビューポートクエリを高速化
CREATE INDEX idx_vm_geom        ON vending_machines USING GIST (geom);
CREATE INDEX idx_vm_geom_active ON vending_machines USING GIST (geom) WHERE is_active = TRUE;
CREATE INDEX idx_vm_osm_id      ON vending_machines (osm_id) WHERE osm_id IS NOT NULL;
CREATE INDEX idx_vm_status      ON vending_machines (status);
```

### machine_votes

```sql
CREATE TABLE machine_votes (
    id                UUID         PRIMARY KEY DEFAULT uuid_generate_v4(),
    machine_id        UUID         NOT NULL REFERENCES vending_machines(id) ON DELETE CASCADE,
    voter_fingerprint TEXT         NOT NULL,   -- SHA256(IP + 日付 + DAILY_SECRET)
    ip_prefix         TEXT,                    -- /24 でマスク済みIPアドレス
    vote_type         TEXT         NOT NULL CHECK (vote_type IN ('exists','removed')),
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_vote_per_fingerprint UNIQUE (machine_id, voter_fingerprint)
);
```

### import_logs

```sql
CREATE TABLE import_logs (
    id             SERIAL       PRIMARY KEY,
    bbox           TEXT         NOT NULL,
    fetched_count  INTEGER      NOT NULL DEFAULT 0,
    inserted_count INTEGER      NOT NULL DEFAULT 0,
    updated_count  INTEGER      NOT NULL DEFAULT 0,
    status         TEXT         NOT NULL,   -- 'running' | 'success' | 'failed'
    error_message  TEXT,
    started_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    finished_at    TIMESTAMPTZ
);
```

## API エンドポイント

| メソッド | パス | 概要 |
|---|---|---|
| GET | `/health` | ヘルスチェック |
| GET | `/api/vending-machines` | bbox内の自販機一覧 (GeoJSON FeatureCollection) |
| POST | `/api/vending-machines` | ユーザーが新規自販機を追加 |
| GET | `/api/vending-machines/:id` | 自販機詳細 |
| PATCH | `/api/vending-machines/:id` | 情報更新 |
| DELETE | `/api/vending-machines/:id` | 非アクティブ化 |
| POST | `/api/vending-machines/:id/votes` | 存在確認投票 |
| POST | `/api/import/osm` | Overpass APIからOSMデータ同期（非同期実行） |
| GET | `/api/import/logs` | 同期履歴一覧 |

### GET /api/vending-machines クエリパラメータ

| パラメータ | 型 | 必須 | 説明 |
|---|---|---|---|
| south | float | ✓ | 南端緯度 |
| west | float | ✓ | 西端経度 |
| north | float | ✓ | 北端緯度 |
| east | float | ✓ | 東端経度 |
| vending_type | string | - | 商品種別フィルタ |
| limit | integer | - | 最大件数 (デフォルト500, 最大1000) |

レスポンスはGeoJSON FeatureCollection形式。propertiesに votes情報 (exists_count, removed_count, trust_score, status) を含む。

### POST /api/vending-machines/:id/votes リクエスト

```json
{ "vote_type": "exists" | "removed" }
```

レスポンス (201):
```json
{
  "machine_id": "uuid",
  "vote_type": "exists",
  "summary": {
    "exists_count": 5,
    "removed_count": 1,
    "trust_score": 62,
    "status": "active",
    "last_confirmed_at": "2026-03-14T10:00:00Z"
  }
}
```

重複投票時は 409、レート超過時は 429 を返す。

## 認証・重複投票防止

ユーザー登録不要。以下の多層防御を実装する。

```
fingerprint = SHA256(IPアドレス + "|" + 今日の日付 + "|" + DAILY_SECRET)
```

- 1日1票制限（日付が変わると再投票可能）
- DBの UNIQUE 制約 (machine_id, fingerprint) が信頼できる防線
- 同一IPプレフィックス (/24) から同一自販機への24時間以内5票超でレート制限
- クライアント側 localStorage に投票済みフラグを保持（UX補助のみ）

## 自販機ステータス自動遷移

投票のたびにリアルタイムで更新する。

| 遷移 | 条件 |
|---|---|
| unverified → active | exists票が1票以上 |
| active → suspected_removed | removed票 ≥ 3 かつ removed > exists × 0.5 |
| suspected_removed → removed | removed票 ≥ 5 かつ 最後のexists票から30日超 |
| removed → active | 新たにexists票が2票以上（復活） |

trust_score = CLAMP(50 + exists_count×2 - removed_count×3, 0, 100)

## フロントエンド機能

- **地図初期化**: Leaflet.js + OSMタイル、デフォルト東京駅
- **マーカー色分け**:
  - 緑: trust_score ≥ 70
  - 黄: trust_score ≥ 40
  - 赤: trust_score < 40
  - 灰: status = removed
  - ★マーク: source = user
- **クラスタリング**: Leaflet.markercluster (CDN)、ズーム16以上で解除
- **ビューポートクエリ**: moveendイベント + 500ms debounce で自動フェッチ
- **追加モード**: ボタンでトグル → 地図クリック → ドラッグ可能仮マーカー + フォーム → POST
- **投票UI**: マーカーポップアップ内に「まだある」「なくなってた」ボタン。投票後スコアを即時更新
- **現在地**: Geolocation API
- **OSM同期**: 現在ビューポートを対象に POST /api/import/osm
- **フィルタ**: 商品種別 select で絞り込み

## Cargo.toml 依存クレート

```toml
axum = { version = "0.7", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower-http = { version = "0.5", features = ["cors", "trace"] }
reqwest = { version = "0.12", features = ["json"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
dotenvy = "0.15"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1"
sha2 = "0.10"
hex = "0.4"
```

## Docker Compose サービス構成

| サービス | イメージ | ポート | 備考 |
|---|---|---|---|
| postgres | postgis/postgis:16-3.4 | 5432 (内部) | migrations/ を initdb で自動実行 |
| backend | docker/backend.Dockerfile | 8080 (内部) | postgres healthy 後に起動 |
| nginx | nginx:alpine | 80 → ホスト | フロントエンド配信 + /api/ プロキシ |

## 環境変数

```
DATABASE_URL=postgres://vending:password@postgres:5432/vending_map
RUST_LOG=info
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
DAILY_SECRET=change-me-in-production
OVERPASS_API_URL=https://overpass-api.de/api/interpreter
```

## エラーコード

| コード | HTTP | 説明 |
|---|---|---|
| NOT_FOUND | 404 | 指定IDが存在しない |
| BAD_REQUEST | 400 | パラメータ不正（bbox逆転、座標範囲外等） |
| ALREADY_VOTED | 409 | 本日すでに投票済み |
| RATE_LIMITED | 429 | IP帯域レート超過 |
| BBOX_TOO_LARGE | 400 | インポートbboxが0.5度超 |
| DATABASE_ERROR | 500 | DB操作失敗 |
| UPSTREAM_ERROR | 502 | Overpass API接続失敗 |
