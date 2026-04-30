CREATE TABLE IF NOT EXISTS vending_machines (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    osm_id BIGINT UNIQUE,
    geom GEOMETRY(Point, 4326) NOT NULL,
    name TEXT,
    vending_type TEXT NOT NULL DEFAULT 'drinks',
    operator TEXT,
    opening_hours TEXT,
    source TEXT NOT NULL DEFAULT 'user',
    osm_tags JSONB,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    trust_score SMALLINT NOT NULL DEFAULT 50,
    status TEXT NOT NULL DEFAULT 'unverified'
        CHECK (status IN ('active', 'suspected_removed', 'removed', 'unverified')),
    confirmed_at TIMESTAMPTZ,
    last_verified DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_vm_geom ON vending_machines USING GIST (geom);
CREATE INDEX IF NOT EXISTS idx_vm_geom_active ON vending_machines USING GIST (geom) WHERE is_active = TRUE;
CREATE INDEX IF NOT EXISTS idx_vm_osm_id ON vending_machines (osm_id) WHERE osm_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_vm_status ON vending_machines (status);
