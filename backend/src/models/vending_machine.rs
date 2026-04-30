use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct VendingMachine {
    pub id: Uuid,
    pub osm_id: Option<i64>,
    pub latitude: f64,
    pub longitude: f64,
    pub name: Option<String>,
    pub vending_type: String,
    pub operator: Option<String>,
    pub opening_hours: Option<String>,
    pub source: String,
    pub osm_tags: Option<Value>,
    pub is_active: bool,
    pub trust_score: i16,
    pub status: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub last_verified: Option<NaiveDate>,
    pub exists_count: i64,
    pub removed_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListVendingMachinesParams {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
    pub vending_type: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVendingMachineRequest {
    pub latitude: f64,
    pub longitude: f64,
    pub name: Option<String>,
    pub vending_type: Option<String>,
    pub operator: Option<String>,
    pub opening_hours: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVendingMachineRequest {
    pub name: Option<String>,
    pub vending_type: Option<String>,
    pub operator: Option<String>,
    pub opening_hours: Option<String>,
    pub is_active: Option<bool>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateVendingMachineInput {
    pub latitude: f64,
    pub longitude: f64,
    pub name: Option<String>,
    pub vending_type: String,
    pub operator: Option<String>,
    pub opening_hours: Option<String>,
    pub source: String,
    pub osm_id: Option<i64>,
    pub osm_tags: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct UpdateVendingMachineInput {
    pub id: Uuid,
    pub name: Option<String>,
    pub vending_type: Option<String>,
    pub operator: Option<String>,
    pub opening_hours: Option<String>,
    pub is_active: Option<bool>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FeatureCollection {
    #[serde(rename = "type")]
    pub feature_type: &'static str,
    pub features: Vec<Feature>,
}

#[derive(Debug, Serialize)]
pub struct Feature {
    #[serde(rename = "type")]
    pub feature_type: &'static str,
    pub geometry: Geometry,
    pub properties: MachineProperties,
}

#[derive(Debug, Serialize)]
pub struct Geometry {
    #[serde(rename = "type")]
    pub geometry_type: &'static str,
    pub coordinates: [f64; 2],
}

#[derive(Debug, Serialize)]
pub struct MachineProperties {
    pub id: Uuid,
    pub osm_id: Option<i64>,
    pub name: Option<String>,
    pub vending_type: String,
    pub operator: Option<String>,
    pub opening_hours: Option<String>,
    pub source: String,
    pub is_active: bool,
    pub trust_score: i16,
    pub status: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub last_verified: Option<NaiveDate>,
    pub exists_count: i64,
    pub removed_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
