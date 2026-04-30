use crate::models::vending_machine::{
    Feature, FeatureCollection, Geometry, MachineProperties, VendingMachine,
};

pub mod health;
pub mod images;
pub mod vending_machine;
pub mod votes;

pub fn to_geojson_feature(machine: VendingMachine) -> Feature {
    Feature {
        feature_type: "Feature",
        geometry: Geometry {
            geometry_type: "Point",
            coordinates: [machine.longitude, machine.latitude],
        },
        properties: MachineProperties {
            id: machine.id,
            osm_id: machine.osm_id,
            name: machine.name,
            vending_type: machine.vending_type,
            operator: machine.operator,
            opening_hours: machine.opening_hours,
            source: machine.source,
            is_active: machine.is_active,
            trust_score: machine.trust_score,
            status: machine.status,
            confirmed_at: machine.confirmed_at,
            last_verified: machine.last_verified,
            exists_count: machine.exists_count,
            removed_count: machine.removed_count,
            created_at: machine.created_at,
            updated_at: machine.updated_at,
        },
    }
}

pub fn to_feature_collection(machines: Vec<VendingMachine>) -> FeatureCollection {
    FeatureCollection {
        feature_type: "FeatureCollection",
        features: machines.into_iter().map(to_geojson_feature).collect(),
    }
}
