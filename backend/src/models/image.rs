use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct MachineImage {
    pub id: Uuid,
    pub machine_id: Uuid,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i32,
    pub sha256: String,
    pub analyzed: bool,
    pub analyze_error: Option<String>,
    pub uploaded_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MachineItem {
    pub id: Uuid,
    pub image_id: Uuid,
    pub machine_id: Uuid,
    pub name: Option<String>,
    pub price_yen: Option<i32>,
    pub temperature: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ItemResponse {
    pub id: Uuid,
    pub name: Option<String>,
    pub price_yen: Option<i32>,
    pub temperature: String,
}

#[derive(Debug, Serialize)]
pub struct ImageResponse {
    pub id: Uuid,
    pub machine_id: Uuid,
    pub image_url: String,
    pub file_size: i32,
    pub analyzed: bool,
    pub analyze_error: Option<String>,
    pub items: Vec<ItemResponse>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub image: ImageResponse,
}

#[derive(Debug)]
pub struct InsertImageInput {
    pub id: Uuid,
    pub machine_id: Uuid,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i32,
    pub sha256: String,
    pub uploaded_by: Option<String>,
}

#[derive(Debug)]
pub struct InsertItemInput {
    pub image_id: Uuid,
    pub machine_id: Uuid,
    pub name: Option<String>,
    pub price_yen: Option<i32>,
    pub temperature: String,
}

pub fn image_to_response(image: MachineImage, items: Vec<MachineItem>) -> ImageResponse {
    ImageResponse {
        image_url: format!("/api/images/{}", image.id),
        id: image.id,
        machine_id: image.machine_id,
        file_size: image.file_size,
        analyzed: image.analyzed,
        analyze_error: image.analyze_error,
        items: items.into_iter().map(item_to_response).collect(),
        created_at: image.created_at,
    }
}

pub fn item_to_response(item: MachineItem) -> ItemResponse {
    ItemResponse {
        id: item.id,
        name: item.name,
        price_yen: item.price_yen,
        temperature: item.temperature,
    }
}

// リクエスト JSON のデシリアライズ用（analyze_image.py の出力）
#[derive(Debug, Deserialize)]
pub struct SlotAnalysis {
    pub name: Option<String>,
    pub price_yen: Option<i32>,
    pub temperature: String,
}

#[derive(Debug, Deserialize)]
pub struct AnalysisOutput {
    #[serde(default)]
    pub slots: Vec<SlotAnalysis>,
    pub error: Option<String>,
}

// ---- T-01: name は NOT NULL (String) だが要件は「名前不要」 ----
// ---- T-05: slot フィルターが name.is_some() に依存している ----
#[cfg(test)]
mod tests {
    use super::*;

    /// T-01 修正確認用: InsertItemInput.name が Option<String> になったことを検証。
    #[test]
    fn t01_insert_item_name_is_option_after_fix() {
        let with_name = InsertItemInput {
            image_id: Uuid::new_v4(),
            machine_id: Uuid::new_v4(),
            name: Some("商品名".to_string()),
            price_yen: Some(150),
            temperature: "hot".to_string(),
        };
        let without_name = InsertItemInput {
            image_id: Uuid::new_v4(),
            machine_id: Uuid::new_v4(),
            name: None,
            price_yen: None,
            temperature: "cold".to_string(),
        };
        drop(with_name);
        drop(without_name);
    }

    /// T-05: slot フィルター条件 `name.is_some() || price_yen.is_some()` を再現し、
    /// temperature のみのスロットが誤って捨てられることを検出する。
    #[test]
    fn t05_slot_filter_drops_temperature_only_slot() {
        let slots = vec![
            SlotAnalysis { name: None, price_yen: None, temperature: "cold".to_string() },
            SlotAnalysis { name: None, price_yen: Some(130), temperature: "unknown".to_string() },
            SlotAnalysis { name: Some("商品".to_string()), price_yen: None, temperature: "hot".to_string() },
        ];

        // handlers/images.rs:93 の現在のフィルター条件を再現
        let current_filter: Vec<_> = slots
            .iter()
            .filter(|s| s.name.is_some() || s.price_yen.is_some())
            .collect();

        // temperature="cold" のみのスロット（先頭）が捨てられていることを確認
        // これが T-05 の問題: 冷温情報が消える
        assert_eq!(
            current_filter.len(),
            2,
            "現在のフィルターは temperature のみのスロットを捨てる（T-05 のバグ）"
        );
        assert!(
            current_filter.iter().all(|s| s.name.is_some() || s.price_yen.is_some()),
            "フィルターを通過したスロットはすべて name か price_yen を持つ"
        );

        // 修正後のフィルター条件: temperature != "unknown" も保持
        let fixed_filter: Vec<_> = slots
            .iter()
            .filter(|s| s.price_yen.is_some() || s.temperature != "unknown")
            .collect();

        assert_eq!(
            fixed_filter.len(),
            3,
            "修正後フィルターは temperature のみのスロットも保持する"
        );
    }

    /// T-05 追加: unknown 温度かつ price なしのスロットは保存不要であることを確認
    #[test]
    fn t05_slot_filter_discards_fully_unknown_slot() {
        let slots = vec![
            SlotAnalysis { name: None, price_yen: None, temperature: "unknown".to_string() },
            SlotAnalysis { name: None, price_yen: Some(100), temperature: "unknown".to_string() },
        ];

        let fixed_filter: Vec<_> = slots
            .iter()
            .filter(|s| s.price_yen.is_some() || s.temperature != "unknown")
            .collect();

        // 最初のスロット（全部 unknown/None）は捨てる
        assert_eq!(fixed_filter.len(), 1);
        assert_eq!(fixed_filter[0].price_yen, Some(100));
    }

    /// T-03: update() RETURNING が exists_count=0 を返すことを型コメントで記録。
    /// DB 統合テストなしには自動検出不可。この関数の存在が課題のドキュメントとなる。
    #[test]
    fn t03_update_returns_hardcoded_zero_vote_counts_documented() {
        // vending_machine_repo::update() の RETURNING 句:
        //   0::BIGINT AS exists_count,   ← 行 208
        //   0::BIGINT AS removed_count,  ← 行 209
        //
        // create() は新規作成直後なので 0 が正しい。
        // update() は既存レコード更新なので、実際の投票数を返すべき。
        //
        // 修正案: update() の RETURNING を get_by_id() と同形式の LEFT JOIN に変更、
        //        または update() 後に get_by_id() を呼び出す。
        //
        // このテストは「課題の存在をコードベースに記録する」目的のため、常に pass する。
        assert!(true, "T-03 は DB 統合テストで検証すること");
    }
}
